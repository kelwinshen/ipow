//! The tunnel API (docs/drafts/ipow-conversion-tunnel.md, Q6): an app asks
//! this operator what it trades and for a quote between two programmable
//! networks; its user then opens the tunnel's buy on the destination, paying
//! its job fees (Q5), and the app registers that buy here. The node's
//! operator takes the buy as any other: it wins it, locks the coin and names
//! the address the user's sell on the source network will pay, and takes
//! that sell on the terms quoted (Q7).
//!
//!   GET  /assets
//!   GET  /quote?from=<network>&fromToken=<native|address>&amount=<smallest units>&to=<network>&token=<native|address>
//!   POST /register  {"to", "swapId", "from", "fromToken", "amountIn", "signature"}
//!
//! A registration is signed by the buy's owner (the wallet that opened it),
//! over `tunnel_message`: nobody else can set the terms of their tunnel.
//!
//! Prices are this operator's own (its settings' coins): what it pays for a
//! coin sold on the source network, and what it asks for a coin bought on
//! the destination.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use ipow_protocol_core::conversion::{ConversionApp, Side, Swap, SwapState};
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::settings::{CoinPrice, TunnelApiSettings};
use ipow_protocol_core::types::Amount;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::secrets::redact;
use crate::swaps::{PAY_BLOCKS, price_ok};
use crate::wallet::SharedWallet;

/// The fewest sats a tunnel moves: the largest standard dust limit (a
/// pay-to-public-key-hash output), so the sell's payment is always relayed.
const DUST: u64 = 546;
/// The largest request body read.
const MAX_BODY: usize = 4096;
/// How long one connection may stay open.
const CONNECTION_TIME: Duration = Duration::from_secs(180);
/// How long a tunnel's promise is kept at most: its buy's auction and
/// funding, and its 12 payment blocks, with room to spare. A buy that is
/// over (done, cancelled, reclaimed) ends it sooner.
const PROMISE_TIME: Duration = Duration::from_secs(6 * 3600);

/// A network's Conversion and this operator's prices there.
pub struct TunnelNetwork {
    pub app: Arc<dyn ConversionApp>,
    pub coins: Vec<CoinPrice>,
}

/// A tunnel this operator registered: the sell it promised to take.
#[derive(Clone, Debug)]
struct Promise {
    from: String,
    /// The coin sold on `from`: `None` for the network's own.
    token_in: Option<String>,
    amount_in: Amount,
    sats: u64,
    to: String,
    token: Option<String>,
    amount_out: Amount,
    buy_id: u64,
    until: Instant,
    /// The sell this node bid on for it: one sell per tunnel, until that
    /// one is refunded.
    sell_id: Option<u64>,
}

/// What the book says of a sell.
#[derive(Debug, PartialEq, Eq)]
pub enum Promised {
    /// Not a tunnel's sell: judged by its price.
    No,
    /// The sell of a tunnel this operator registered, on the terms it
    /// quoted: taken whatever the price is now.
    Yes,
    /// Never taken: a sell with a payment window that is not the sell of a
    /// tunnel this node registered on its terms (a late payment would refund
    /// it and leave the operator's BTC paid), or a second sell paying a
    /// tunnel's address (the buy is completed once).
    Refuse,
}

/// Where a job is anchored on Bitcoin: the height of its anchor block.
#[async_trait]
trait Anchors: Send + Sync {
    async fn anchor_height(&self, job_id: u64) -> Option<u32>;
}

struct NetworkAnchors(Arc<dyn ProtocolNetwork>);

#[async_trait]
impl Anchors for NetworkAnchors {
    async fn anchor_height(&self, job_id: u64) -> Option<u32> {
        Some(self.0.job(job_id).await.ok()?.anchor?.height)
    }
}

/// A network the book reads: its Conversion, and its protocol for the buys'
/// anchors.
struct BookNetwork {
    app: Arc<dyn ConversionApp>,
    anchors: Arc<dyn Anchors>,
}

/// The tunnels this node registered, shared by the API and the operator on
/// every network. Kept in memory: after a restart a tunnel's sell is
/// refused, and its user refunded.
#[derive(Default)]
pub struct TunnelBook {
    networks: SyncMutex<HashMap<String, Arc<BookNetwork>>>,
    promises: SyncMutex<Vec<Promise>>,
}

/// A buy's payment blocks: the 12 after its job's anchor.
async fn pay_blocks(n: &BookNetwork, buy: &Swap) -> Option<(u32, u32)> {
    let anchor = n.anchors.anchor_height(buy.job_id).await?;
    Some((anchor + 1, anchor + PAY_BLOCKS))
}

impl TunnelBook {
    pub fn add_network(&self, name: &str, app: Arc<dyn ConversionApp>, net: Arc<dyn ProtocolNetwork>) {
        self.add(name, app, Arc::new(NetworkAnchors(net)));
    }

    fn add(&self, name: &str, app: Arc<dyn ConversionApp>, anchors: Arc<dyn Anchors>) {
        self.networks.lock().unwrap().insert(name.to_string(), Arc::new(BookNetwork { app, anchors }));
    }

    fn network(&self, name: &str) -> Option<Arc<BookNetwork>> {
        self.networks.lock().unwrap().get(name).cloned()
    }

    fn promise(&self, p: Promise) {
        self.promises.lock().unwrap().push(p);
    }

    /// Whether a buy is a tunnel's already.
    fn has(&self, to: &str, buy_id: u64) -> bool {
        self.open_promises().iter().any(|x| x.to == to && x.buy_id == buy_id)
    }

    fn open_promises(&self) -> Vec<Promise> {
        let now = Instant::now();
        let mut p = self.promises.lock().unwrap();
        p.retain(|x| x.until > now);
        p.clone()
    }

    /// The tunnel whose buy's address `sell` pays, with the buy.
    async fn tunnel_of(&self, network: &str, sell: &Swap) -> Option<(Promise, Swap, Arc<BookNetwork>)> {
        for c in self.open_promises().into_iter().filter(|x| x.from == network) {
            let Some(to) = self.network(&c.to) else { continue };
            let Ok(buy) = to.app.swap(c.buy_id).await else { continue };
            if buy.side == Side::Buy && !buy.script.is_empty() && buy.script == sell.script {
                return Some((c, buy, to));
            }
        }
        None
    }

    /// Whether `sell`, on `network`, is a tunnel's sell this operator
    /// promised to take: paying the funded buy's address, in exactly the
    /// buy's payment blocks, the coin quoted, at least the amount quoted,
    /// exactly the sats, and no other live sell bid on for the tunnel.
    pub async fn promised(&self, network: &str, sell: &Swap) -> Promised {
        if sell.side != Side::Sell {
            return Promised::No;
        }
        let Some((c, buy, to)) = self.tunnel_of(network, sell).await else {
            return if sell.pay_window.is_some() { Promised::Refuse } else { Promised::No };
        };
        let terms = buy.state == SwapState::Funded
            && same_token(sell.token.as_deref(), c.token_in.as_deref())
            && sell.sats == c.sats
            && sell.amount >= c.amount_in
            && sell.pay_window.is_some()
            && sell.pay_window == pay_blocks(&to, &buy).await;
        if !terms {
            return Promised::Refuse;
        }
        match c.sell_id {
            Some(id) if id != sell.id => {
                // The other sell, refunded or cancelled, frees the tunnel.
                let Some(from) = self.network(network) else { return Promised::Refuse };
                match from.app.swap(id).await {
                    Ok(other) if matches!(other.state, SwapState::Refunded | SwapState::Cancelled) => Promised::Yes,
                    _ => Promised::Refuse,
                }
            }
            _ => Promised::Yes,
        }
    }

    /// Marks the tunnel `sell` pays as taken by it, once this node bid on
    /// it; a tunnel another live sell took first keeps that one (`promised`
    /// then refuses this one, and its payment is not sent).
    pub async fn take(&self, network: &str, sell: &Swap) {
        if self.promised(network, sell).await != Promised::Yes {
            return;
        }
        if let Some((c, _, _)) = self.tunnel_of(network, sell).await
            && let Some(x) = self.promises.lock().unwrap().iter_mut().find(|x| x.to == c.to && x.buy_id == c.buy_id)
        {
            x.sell_id = Some(sell.id);
        }
    }

    /// The promises whose buys are still live, with each buy's state; a
    /// buy that is over is forgotten.
    async fn live_promises(&self) -> Vec<(Promise, SwapState)> {
        let mut live = vec![];
        let mut over = vec![];
        for c in self.open_promises() {
            let Some(n) = self.network(&c.to) else { continue };
            match n.app.swap(c.buy_id).await {
                Ok(b) if matches!(b.state, SwapState::Open | SwapState::Funded) => live.push((c, b.state)),
                Ok(_) => over.push((c.to.clone(), c.buy_id)),
                Err(_) => live.push((c, SwapState::Open)),
            }
        }
        if !over.is_empty() {
            self.promises.lock().unwrap().retain(|x| !over.iter().any(|(to, id)| x.to == *to && x.buy_id == *id));
        }
        live
    }

    /// The sats of every tunnel still live: what the wallet may yet have to
    /// pay for their sells.
    async fn promised_sats(&self) -> u64 {
        self.live_promises().await.iter().map(|(p, _)| p.sats).sum()
    }

    /// The coin promised on `to` to buys not funded yet.
    async fn promised_out(&self, to: &str, token: Option<&str>) -> Amount {
        self.live_promises()
            .await
            .into_iter()
            .filter(|(p, state)| p.to == to && same_token(p.token.as_deref(), token) && *state == SwapState::Open)
            .fold(0, |sum: Amount, (p, _)| sum.saturating_add(p.amount_out))
    }
}

pub struct Tunnels {
    networks: HashMap<String, TunnelNetwork>,
    settings: TunnelApiSettings,
    key: Option<String>,
    book: Arc<TunnelBook>,
    /// The operator's Bitcoin wallet: it pays each tunnel's sell first.
    wallet: Arc<SharedWallet>,
    /// One tunnel registered at a time: the balances it checks then count
    /// what the one before promised.
    registering: Mutex<()>,
}

/// A quote: the sats between the two networks, and what the user receives.
#[derive(Debug, PartialEq, Eq)]
pub struct Quote {
    pub sats: u64,
    pub amount_out: Amount,
}

/// A fee in a network's coin as sats, at what this operator asks for the
/// coin, rounded up.
pub fn fee_sats(fee: Amount, coin: &CoinPrice) -> Option<u64> {
    let unit = 10u128.checked_pow(coin.decimals as u32)?;
    let v = fee.checked_mul(coin.ask_sats as u128)?;
    u64::try_from(v.div_ceil(unit)).ok()
}

/// Selling `amount_in` of `from` (smallest units) at what this operator pays
/// for it, then buying `to` with those sats less `fee_sats` (the buy's job
/// fees) at what it asks: both rounded down, so the operator never gives
/// more than its prices. The buy is for all the sats: the user's sell pays
/// them, and the buy's receipt spends them.
pub fn quote(from: &CoinPrice, amount_in: Amount, to: &CoinPrice, fee_sats: u64) -> Option<Quote> {
    let unit_in = 10u128.checked_pow(from.decimals as u32)?;
    let sats = amount_in.checked_mul(from.pay_sats as u128)? / unit_in;
    let sats = u64::try_from(sats).ok()?;
    let left = sats.checked_sub(fee_sats).filter(|l| *l > 0)?;
    let unit_out = 10u128.checked_pow(to.decimals as u32)?;
    if to.ask_sats == 0 {
        return None;
    }
    let amount_out = (left as u128).checked_mul(unit_out)? / to.ask_sats as u128;
    (amount_out > 0).then_some(Quote { sats, amount_out })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterRequest {
    /// The destination network and the buy the user opened there.
    to: String,
    swap_id: u64,
    /// The source network, and what is sold there.
    from: String,
    #[serde(default)]
    from_token: Option<String>,
    amount_in: String,
    /// The buy's owner's signature over `tunnel_message`.
    signature: String,
}

/// What the buy's owner signs to register a tunnel: the same words in the
/// app (tunnel.ts) and here.
pub fn tunnel_message(to: &str, swap_id: u64, from: &str, from_token: &str, amount_in: &str) -> String {
    format!("iPoW tunnel: buy {swap_id} on {to}; sell {amount_in} of {from_token} on {from}")
}

/// Two tokens as a network writes them: EVM addresses in any case, others
/// exactly; `None` is the network's own coin.
fn same_token(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => if a.starts_with("0x") { a.eq_ignore_ascii_case(b) } else { a == b },
        _ => false,
    }
}

fn coin<'a>(coins: &'a [CoinPrice], token: Option<&str>) -> Option<&'a CoinPrice> {
    coins.iter().find(|c| same_token(if c.token == "native" { None } else { Some(&c.token) }, token))
}

/// A token as the API and the settings name it: `native` for the coin.
fn token_of(name: Option<&str>) -> Option<&str> {
    name.filter(|t| *t != "native")
}

/// Two keys compared in a time that does not depend on where they differ.
fn same_key(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

type Failure = (StatusCode, String);

fn bad(m: &str) -> Failure {
    (StatusCode::BAD_REQUEST, m.to_string())
}

impl Tunnels {
    pub fn new(networks: HashMap<String, TunnelNetwork>, settings: TunnelApiSettings, key: Option<String>, book: Arc<TunnelBook>, wallet: Arc<SharedWallet>) -> Self {
        Tunnels { networks, settings, key, book, wallet, registering: Mutex::new(()) }
    }

    /// Serves the API until the process stops.
    pub async fn serve(self: Arc<Self>) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(&self.settings.listen).await?;
        info!(listen = %self.settings.listen, keyed = self.key.is_some(), "tunnel API");
        loop {
            let (stream, _) = listener.accept().await?;
            let me = self.clone();
            tokio::spawn(async move {
                let service = service_fn(move |req| {
                    let me = me.clone();
                    async move { Ok::<_, Infallible>(me.handle(req).await) }
                });
                let conn = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), service);
                match tokio::time::timeout(CONNECTION_TIME, conn).await {
                    Ok(Err(e)) => warn!(error = %e, "tunnel API connection"),
                    Err(_) => warn!("tunnel API connection timed out"),
                    Ok(Ok(())) => {}
                }
            });
        }
    }

    async fn handle(&self, req: Request<Incoming>) -> Response<Full<Bytes>> {
        let (method, path, query) = (req.method().clone(), req.uri().path().to_string(), req.uri().query().unwrap_or("").to_string());
        let allowed = match &self.key {
            None => true,
            Some(k) => req.headers().get("x-tunnel-key").and_then(|v| v.to_str().ok()).is_some_and(|v| same_key(v, k)),
        };
        let result = match (method, path.as_str()) {
            (Method::OPTIONS, _) => Ok(json!({})),
            _ if !allowed => Err((StatusCode::UNAUTHORIZED, "a key is needed".to_string())),
            (Method::GET, "/assets") => Ok(self.assets()),
            (Method::GET, "/quote") => self.quote(&query).await,
            (Method::POST, "/register") => match Limited::new(req.into_body(), MAX_BODY).collect().await {
                Ok(body) => self.register(&body.to_bytes()).await,
                Err(_) => Err(bad("unreadable or too large body")),
            },
            _ => Err((StatusCode::NOT_FOUND, "no such route".to_string())),
        };
        let (status, body) = match result {
            Ok(v) => (StatusCode::OK, v),
            Err((status, message)) => (status, json!({ "error": message })),
        };
        Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .header("access-control-allow-origin", "*")
            .header("access-control-allow-methods", "GET, POST, OPTIONS")
            .header("access-control-allow-headers", "content-type, x-tunnel-key")
            .body(Full::new(Bytes::from(body.to_string())))
            .expect("a valid response")
    }

    /// What this operator trades, with its prices: what an app lists.
    fn assets(&self) -> Value {
        let mut names: Vec<&String> = self.networks.keys().collect();
        names.sort();
        let networks: Vec<Value> = names
            .into_iter()
            .map(|name| {
                let coins: Vec<Value> = self.networks[name]
                    .coins
                    .iter()
                    .map(|c| {
                        json!({
                            "token": c.token,
                            "symbol": c.symbol.clone().unwrap_or_else(|| c.token.clone()),
                            "decimals": c.decimals,
                            "paySats": c.pay_sats,
                            "askSats": c.ask_sats,
                        })
                    })
                    .collect();
                json!({ "name": name, "coins": coins })
            })
            .collect();
        json!({ "networks": networks, "maxSats": self.settings.max_sats })
    }

    /// The quote for selling `amount_in` of `from_token` on `from` into
    /// `token` on `to`.
    async fn quote_for(&self, from: &str, from_token: Option<&str>, amount_in: Amount, to: &str, token: Option<&str>) -> Result<Quote, Failure> {
        let from_net = self.networks.get(from).ok_or_else(|| bad("this operator does not serve that network"))?;
        let to_net = self.networks.get(to).ok_or_else(|| bad("this operator does not serve that network"))?;
        if from == to {
            return Err(bad("a tunnel is between two networks"));
        }
        let from_coin = coin(&from_net.coins, from_token).ok_or_else(|| bad("this operator does not buy that coin"))?;
        let to_coin = coin(&to_net.coins, token).ok_or_else(|| bad("this operator does not sell that token"))?;
        // The buy's job fees are paid in the destination's coin.
        let native = coin(&to_net.coins, None).ok_or_else(|| bad("this operator has no price for that network's coin"))?;
        let fee = if self.settings.price_fees {
            let fee = to_net.app.buy_fees().await.map_err(|e| {
                warn!(network = %to, error = %redact(&e), "tunnel: could not read the fees");
                (StatusCode::BAD_GATEWAY, "could not read the fees; try again".to_string())
            })?;
            fee_sats(fee, native).ok_or_else(|| bad("fees out of range"))?
        } else {
            0
        };
        let q = quote(from_coin, amount_in, to_coin, fee).ok_or_else(|| bad("this amount is too small, or out of range"))?;
        // Below Bitcoin's dust the sell could not be paid: the buy would be
        // opened and funded for nothing.
        if q.sats < DUST {
            return Err(bad(&format!("at least {DUST} sats per tunnel")));
        }
        if q.sats > self.settings.max_sats {
            return Err(bad(&format!("at most {} sats per tunnel", self.settings.max_sats)));
        }
        Ok(q)
    }

    async fn quote(&self, query: &str) -> Result<Value, Failure> {
        let q: HashMap<&str, &str> = query.split('&').filter_map(|kv| kv.split_once('=')).collect();
        let from = *q.get("from").ok_or_else(|| bad("from is missing"))?;
        let to = *q.get("to").ok_or_else(|| bad("to is missing"))?;
        let amount: Amount = q.get("amount").and_then(|a| a.parse().ok()).ok_or_else(|| bad("amount must be a whole number"))?;
        let token = token_of(q.get("token").copied());
        let from_token = token_of(q.get("fromToken").copied());
        let quote = self.quote_for(from, from_token, amount, to, token).await?;
        Ok(json!({ "sats": quote.sats, "amountOut": quote.amount_out.to_string() }))
    }

    /// Takes a buy the user opened on the destination as a tunnel's: on
    /// this operator's terms for both legs, within its means, and not
    /// registered before. From then on its sell is promised (Q7).
    async fn register(&self, body: &[u8]) -> Result<Value, Failure> {
        let r: RegisterRequest = serde_json::from_slice(body).map_err(|e| bad(&format!("bad request: {e}")))?;
        let to = self.networks.get(&r.to).ok_or_else(|| bad("this operator does not serve that network"))?;
        let from = self.networks.get(&r.from).ok_or_else(|| bad("this operator does not serve that network"))?;
        if r.from == r.to {
            return Err(bad("a tunnel is between two networks"));
        }
        let from_token = token_of(r.from_token.as_deref());
        let amount_in: Amount = r.amount_in.parse().map_err(|_| bad("amountIn must be a whole number"))?;
        let buy = to.app.swap(r.swap_id).await.map_err(|_| bad("no such swap on that network"))?;
        if buy.side != Side::Buy || !matches!(buy.state, SwapState::Open | SwapState::Funded) {
            return Err(bad("that swap is not an open buy"));
        }
        // Only the buy's owner sets its tunnel's terms.
        let message = tunnel_message(&r.to, r.swap_id, &r.from, r.from_token.as_deref().unwrap_or("native"), &r.amount_in);
        if !to.app.signed_by(&buy.user, &message, &r.signature) {
            return Err((StatusCode::FORBIDDEN, "the signature is not the buy's owner's".to_string()));
        }
        if buy.sats < DUST || buy.sats > self.settings.max_sats {
            return Err(bad(&format!("a tunnel is {DUST} to {} sats", self.settings.max_sats)));
        }
        // Both legs at this operator's prices now: what the buy asks of it,
        // and what the sell will pay it.
        let to_price = coin(&to.coins, buy.token.as_deref()).ok_or_else(|| bad("this operator does not sell that token"))?;
        let from_price = coin(&from.coins, from_token).ok_or_else(|| bad("this operator does not buy that coin"))?;
        if !price_ok(Side::Buy, buy.sats, buy.amount, to_price) || !price_ok(Side::Sell, buy.sats, amount_in, from_price) {
            return Err((StatusCode::CONFLICT, "not at this operator's price any more: it will not take this tunnel".to_string()));
        }
        let _one_at_a_time = self.registering.lock().await;
        // Registered again by its owner: the terms are the newest, unless a
        // sell was already bid on for the old ones.
        if self.book.has(&r.to, r.swap_id) {
            let taken = self.book.open_promises().into_iter().any(|x| x.to == r.to && x.buy_id == r.swap_id && x.sell_id.is_some());
            if taken {
                return Err((StatusCode::CONFLICT, "a sell for this tunnel was already taken on its earlier terms".to_string()));
            }
            self.book.promises.lock().unwrap().retain(|x| !(x.to == r.to && x.buy_id == r.swap_id));
        }
        // The BTC to pay the sell with, on top of the tunnels still live.
        let btc_ok = self
            .wallet
            .can_pay_sats(buy.sats.saturating_add(self.book.promised_sats().await))
            .await
            .map_err(|_| (StatusCode::BAD_GATEWAY, "could not read the Bitcoin wallet; try again".to_string()))?;
        if !btc_ok {
            return Err((StatusCode::SERVICE_UNAVAILABLE, "this operator's Bitcoin wallet cannot pay for another tunnel now".to_string()));
        }
        // The coin to lock, with a twentieth more as the operator keeps, on
        // top of what it promised to buys it has not funded yet.
        if buy.state == SwapState::Open {
            let token = buy.token.as_deref();
            let have = to.app.my_balance(token).await.map_err(|_| (StatusCode::BAD_GATEWAY, "could not read the balance; try again".to_string()))?;
            let need = buy.amount.saturating_add(buy.amount / 20).saturating_add(self.book.promised_out(&r.to, token).await);
            if have < need {
                return Err((StatusCode::SERVICE_UNAVAILABLE, "this operator holds too little of that coin now".to_string()));
            }
        }
        self.book.promise(Promise {
            from: r.from.clone(),
            token_in: from_token.map(str::to_string),
            amount_in,
            sats: buy.sats,
            to: r.to.clone(),
            token: buy.token.clone(),
            amount_out: buy.amount,
            buy_id: r.swap_id,
            until: Instant::now() + PROMISE_TIME,
            sell_id: None,
        });
        info!(from = %r.from, network = %r.to, swap = r.swap_id, sats = buy.sats, "tunnel: the user's buy registered; its sell is promised");
        Ok(json!({ "network": r.to, "swapId": r.swap_id, "sats": buy.sats, "amountOut": buy.amount.to_string() }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price(token: &str, decimals: u8, pay_sats: u64, ask_sats: u64) -> CoinPrice {
        CoinPrice { token: token.into(), symbol: None, decimals, pay_sats, ask_sats }
    }

    #[test]
    fn finds_a_coin_by_its_token() {
        let coins = [price("native", 18, 1, 1), price("0xAbC0000000000000000000000000000000000001", 18, 2, 2)];
        assert_eq!(coin(&coins, None).map(|c| c.pay_sats), Some(1));
        assert_eq!(coin(&coins, Some("0xabc0000000000000000000000000000000000001")).map(|c| c.pay_sats), Some(2));
        assert!(coin(&coins, Some("0xabc0000000000000000000000000000000000002")).is_none());
        assert_eq!(token_of(Some("native")), None);
        assert_eq!(token_of(Some("0x1")), Some("0x1"));
    }

    #[test]
    fn quotes_through_the_sats_at_the_operators_prices() {
        // It pays 2,000,000 sats for 1 SOL; asks 3,000,000 sats for 1 ETH.
        let sol = price("native", 9, 2_000_000, 2_100_000);
        let eth = price("native", 18, 2_900_000, 3_000_000);
        // 0.1 SOL: 200,000 sats, then 200,000 / 3,000,000 of an ETH.
        let q = quote(&sol, 100_000_000, &eth, 0).unwrap();
        assert_eq!(q.sats, 200_000);
        assert_eq!(q.amount_out, 200_000u128 * 10u128.pow(18) / 3_000_000);
        // The fees come off what the user receives, not off the sats.
        let q = quote(&sol, 100_000_000, &eth, 1_000).unwrap();
        assert_eq!(q.sats, 200_000);
        assert_eq!(q.amount_out, 199_000u128 * 10u128.pow(18) / 3_000_000);
        // Refused when the fees take it all, or a price is missing.
        assert!(quote(&sol, 100_000_000, &eth, 200_000).is_none());
        assert!(quote(&sol, 1, &eth, 0).is_none());
        assert!(quote(&sol, 100_000_000, &price("native", 18, 1, 0), 0).is_none());
    }

    #[test]
    fn fees_in_sats_round_up() {
        let eth = price("native", 18, 2_900_000, 3_000_000);
        // 0.001 ETH at 3,000,000 sats an ETH: 3,000 sats.
        assert_eq!(fee_sats(10u128.pow(15), &eth), Some(3_000));
        assert_eq!(fee_sats(1, &eth), Some(1));
        assert_eq!(fee_sats(0, &eth), Some(0));
    }

    #[test]
    fn keys_compare_whole() {
        assert!(same_key("abc", "abc"));
        assert!(!same_key("abc", "abd"));
        assert!(!same_key("abc", "abcd"));
    }

    /// A Conversion holding some swaps, whose states a test may change.
    struct Swaps(SyncMutex<Vec<Swap>>);

    #[async_trait]
    impl ConversionApp for Swaps {
        fn application(&self) -> String {
            "app".into()
        }
        async fn swaps_after(&self, _: u64, _: usize) -> anyhow::Result<Vec<Swap>> {
            Ok(vec![])
        }
        async fn swap(&self, id: u64) -> anyhow::Result<Swap> {
            self.0.lock().unwrap().iter().find(|s| s.id == id).cloned().ok_or_else(|| anyhow::anyhow!("no such swap"))
        }
        async fn my_balance(&self, _: Option<&str>) -> anyhow::Result<Amount> {
            Ok(0)
        }
        async fn buy_fees(&self) -> anyhow::Result<Amount> {
            Ok(0)
        }
        fn signed_by(&self, _: &str, _: &str, _: &str) -> bool {
            true
        }
        async fn fund(&self, _: u64, _: &[u8]) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn complete_sell(&self, _: u64, _: &[u8]) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn complete_buy(&self, _: u64, _: &[u8], _: &[u8], _: u32) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn reclaim(&self, _: u64) -> anyhow::Result<()> {
            unimplemented!()
        }
    }

    /// Every job anchored at height 100: a buy's payment blocks are 101 to 112.
    struct At100;

    #[async_trait]
    impl Anchors for At100 {
        async fn anchor_height(&self, _: u64) -> Option<u32> {
            Some(100)
        }
    }

    fn swap(id: u64, side: Side, state: SwapState, amount: Amount, sats: u64, script: &[u8], window: Option<(u32, u32)>) -> Swap {
        Swap { id, side, state, user: "u".into(), token: None, amount, sats, job_id: id, script: script.to_vec(), pay_window: window }
    }

    #[tokio::test]
    async fn keeps_its_promise_to_one_sell_on_the_terms_quoted() {
        let s = [0x00, 0x14, 7, 7];
        let w = Some((101, 112));
        let sell = |id, amount, sats, script: &[u8], window| swap(id, Side::Sell, SwapState::Open, amount, sats, script, window);
        let book = TunnelBook::default();
        let a = Arc::new(Swaps(SyncMutex::new(vec![sell(9, 1_000, 50_000, &s, w), sell(10, 1_000, 50_000, &s, w)])));
        book.add("a", a.clone(), Arc::new(At100));
        book.add("b", Arc::new(Swaps(SyncMutex::new(vec![swap(4, Side::Buy, SwapState::Funded, 10, 50_000, &s, None)]))), Arc::new(At100));
        book.promise(Promise {
            from: "a".into(),
            token_in: None,
            amount_in: 1_000,
            sats: 50_000,
            to: "b".into(),
            token: None,
            amount_out: 10,
            buy_id: 4,
            until: Instant::now() + PROMISE_TIME,
            sell_id: None,
        });
        assert!(book.has("b", 4));
        assert!(!book.has("b", 5));
        assert_eq!(book.promised_sats().await, 50_000);
        assert_eq!(book.promised_out("b", None).await, 0);
        assert_eq!(tunnel_message("b", 4, "a", "native", "1000"), "iPoW tunnel: buy 4 on b; sell 1000 of native on a");
        // A plain sell elsewhere: judged by its price. A sell with a window
        // that is no tunnel of this node's: refused.
        assert_eq!(book.promised("a", &sell(9, 1_000, 50_000, &[1], None)).await, Promised::No);
        assert_eq!(book.promised("a", &sell(9, 1_000, 50_000, &[1], w)).await, Promised::Refuse);
        assert_eq!(book.promised("c", &sell(9, 1_000, 50_000, &s, w)).await, Promised::Refuse);
        // Paying the tunnel's address on other terms: another window, none,
        // other sats, too little.
        assert_eq!(book.promised("a", &sell(9, 1_000, 50_000, &s, Some((101, 103)))).await, Promised::Refuse);
        assert_eq!(book.promised("a", &sell(9, 1_000, 50_000, &s, None)).await, Promised::Refuse);
        assert_eq!(book.promised("a", &sell(9, 1_000, 49_999, &s, w)).await, Promised::Refuse);
        assert_eq!(book.promised("a", &sell(9, 999, 50_000, &s, w)).await, Promised::Refuse);
        // The tunnel's sell: taken. Until the node bids on one, any matching sell is.
        assert_eq!(book.promised("a", &sell(9, 1_000, 50_000, &s, w)).await, Promised::Yes);
        assert_eq!(book.promised("a", &sell(10, 1_000, 50_000, &s, w)).await, Promised::Yes);
        book.take("a", &sell(9, 1_000, 50_000, &s, w)).await;
        assert_eq!(book.promised("a", &sell(9, 1_000, 50_000, &s, w)).await, Promised::Yes);
        // A second sell to the same address, on any terms: refused...
        assert_eq!(book.promised("a", &sell(10, 1_000, 50_000, &s, w)).await, Promised::Refuse);
        assert_eq!(book.promised("a", &sell(10, 5_000, 60_000, &s, w)).await, Promised::Refuse);
        // Bidding on it too does not move the tunnel to it.
        book.take("a", &sell(10, 1_000, 50_000, &s, w)).await;
        assert_eq!(book.promised("a", &sell(9, 1_000, 50_000, &s, w)).await, Promised::Yes);
        assert_eq!(book.promised("a", &sell(10, 1_000, 50_000, &s, w)).await, Promised::Refuse);
        // ...until the first is refunded.
        a.0.lock().unwrap()[0].state = SwapState::Refunded;
        assert_eq!(book.promised("a", &sell(10, 1_000, 50_000, &s, w)).await, Promised::Yes);
    }
}
