//! The tunnel API (docs/drafts/ipow-conversion-tunnel.md, Q6): an app asks
//! this operator what it trades and for a quote between two programmable
//! networks; its user then opens the tunnel's buy on the destination, paying
//! its job fees (Q5), with the sale's terms in the buy's memo (the SDK's
//! `tunnelMemo`). The node's operator reads the memo as it considers the
//! buy (`TunnelBook::consider_buy`): on its terms and within its means it
//! promises the sale (Q7), then takes the buy as any other, locks the coin
//! and names the address the sale on the source network will pay.
//!
//!   GET  /assets
//!   GET  /quote?from=<network>&fromToken=<native|address>&amount=<smallest units>&to=<network>&token=<native|address>
//!
//! Prices are this operator's own (its settings' coins): what it pays for a
//! coin sold on the source network, and what it asks for a coin bought on
//! the destination.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use ipow_protocol_core::conversion::{ConversionApp, Side, Swap, SwapState};
use ipow_protocol_core::network::ProtocolNetwork;
use ipow_protocol_core::settings::{CoinPrice, TunnelApiSettings};
use ipow_protocol_core::types::Amount;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::secrets::redact;
use crate::swaps::{PAY_BLOCKS, price_ok};
use crate::wallet::SharedWallet;

/// The fewest sats a tunnel moves: the largest standard dust limit (a
/// pay-to-public-key-hash output), so the sell's payment is always relayed.
const DUST: u64 = 546;
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

/// A tunnel this operator promised: the sell it will take.
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
    /// The sell of a tunnel this operator promised, on the terms it
    /// quoted: taken whatever the price is now.
    Yes,
    /// Never taken: a sell with a payment window that is not the sell of a
    /// tunnel this node promised on its terms (a late payment would refund
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

/// A network the book reads: its Conversion, its protocol for the buys'
/// anchors, and this operator's prices there.
struct BookNetwork {
    app: Arc<dyn ConversionApp>,
    anchors: Arc<dyn Anchors>,
    coins: Vec<CoinPrice>,
}

/// What a buy's memo says of a tunnel.
#[derive(Debug, PartialEq, Eq)]
pub enum Memo {
    /// No tunnel memo: an ordinary buy.
    None,
    /// A tunnel's terms (the SDK's `tunnelMemo`): the source network, the
    /// coin sold there (`None` for its own), the amount in.
    Tunnel(String, Option<String>, Amount),
    /// Claims to be a tunnel's, but cannot be read: a version this node does
    /// not know, or malformed. Its user expects a tunnel, so the buy is not
    /// taken as an ordinary one.
    Unreadable,
}

/// A buy's memo, as the SDK writes it: `ipow-tunnel/1 <source network>
/// <native|token> <amount in>`, single spaces, decimal digits.
pub fn parse_memo(memo: &[u8]) -> Memo {
    let Ok(text) = std::str::from_utf8(memo) else { return Memo::Unreadable };
    if !text.starts_with("ipow-tunnel/") {
        return Memo::None;
    }
    let words: Vec<&str> = text.split(' ').collect();
    if words.len() != 4 || words[0] != "ipow-tunnel/1" || words[1].is_empty() || words[2].is_empty() {
        return Memo::Unreadable;
    }
    let digits = !words[3].is_empty() && words[3].bytes().all(|b| b.is_ascii_digit());
    match (digits, words[3].parse::<Amount>()) {
        (true, Ok(amount_in)) => Memo::Tunnel(words[1].to_string(), token_of(Some(words[2])).map(str::to_string), amount_in),
        _ => Memo::Unreadable,
    }
}

/// The tunnels this node promised, shared by the API and the operator on
/// every network. Kept in memory: after a restart a tunnel's sell is
/// refused, and its user refunded.
pub struct TunnelBook {
    networks: SyncMutex<HashMap<String, Arc<BookNetwork>>>,
    promises: SyncMutex<Vec<Promise>>,
    /// The most sats one tunnel moves.
    max_sats: u64,
    /// One buy considered at a time: the balances it checks then count
    /// what the one before promised.
    considering: Mutex<()>,
}

/// A buy's payment blocks: the 12 after its job's anchor.
async fn pay_blocks(n: &BookNetwork, buy: &Swap) -> Option<(u32, u32)> {
    let anchor = n.anchors.anchor_height(buy.job_id).await?;
    Some((anchor + 1, anchor + PAY_BLOCKS))
}

impl TunnelBook {
    pub fn new(max_sats: u64) -> Self {
        TunnelBook { networks: SyncMutex::new(HashMap::new()), promises: SyncMutex::new(vec![]), max_sats, considering: Mutex::new(()) }
    }

    pub fn add_network(&self, name: &str, app: Arc<dyn ConversionApp>, net: Arc<dyn ProtocolNetwork>, coins: Vec<CoinPrice>) {
        self.add(name, app, Arc::new(NetworkAnchors(net)), coins);
    }

    fn add(&self, name: &str, app: Arc<dyn ConversionApp>, anchors: Arc<dyn Anchors>, coins: Vec<CoinPrice>) {
        self.networks.lock().unwrap().insert(name.to_string(), Arc::new(BookNetwork { app, anchors, coins }));
    }

    /// Whether this operator can take `buy`, on `network`, as a tunnel's: its
    /// memo names a sale this operator serves, at its prices for both legs,
    /// within `max_sats`, and within what it can fund and pay (the BTC for
    /// the sale, on top of the tunnels still live). A buy with no tunnel
    /// memo is nobody's tunnel: true, and judged as any buy. Refused, the
    /// buy is left to expire. The promise itself is made once the node has
    /// bid (`promise_buy`), so a bid that does not happen leaves none.
    pub async fn can_take_buy(&self, network: &str, buy: &Swap, wallet: &SharedWallet) -> bool {
        let (from, token_in, amount_in) = match parse_memo(&buy.memo) {
            Memo::None => return true,
            Memo::Unreadable => {
                warn!(network, swap = buy.id, "tunnel: the buy's memo claims a tunnel this node cannot read; not taken");
                return false;
            }
            Memo::Tunnel(from, token_in, amount_in) => (from, token_in, amount_in),
        };
        if buy.side != Side::Buy || !matches!(buy.state, SwapState::Open | SwapState::Funded) {
            return false;
        }
        let refuse = |why: &str| {
            warn!(network, swap = buy.id, from = %from, why, "tunnel: the buy's memo names a sale this operator will not take");
            false
        };
        if from == network {
            return refuse("a tunnel is between two networks");
        }
        let (Some(to), Some(src)) = (self.network(network), self.network(&from)) else { return refuse("a network this operator does not serve") };
        if buy.sats < DUST || buy.sats > self.max_sats {
            return refuse("sats outside this operator's range");
        }
        let (Some(to_price), Some(from_price)) = (coin(&to.coins, buy.token.as_deref()), coin(&src.coins, token_in.as_deref())) else {
            return refuse("a coin this operator does not trade");
        };
        if !price_ok(Side::Buy, buy.sats, buy.amount, to_price) || !price_ok(Side::Sell, buy.sats, amount_in, from_price) {
            return refuse("not at this operator's prices");
        }
        let _one_at_a_time = self.considering.lock().await;
        if self.has(network, buy.id) {
            return true;
        }
        match wallet.can_pay_sats(buy.sats.saturating_add(self.promised_sats().await)).await {
            Ok(true) => {}
            Ok(false) => return refuse("the Bitcoin wallet cannot pay for another tunnel now"),
            Err(_) => return refuse("the Bitcoin wallet could not be read"),
        }
        if buy.state == SwapState::Open {
            let Ok(have) = to.app.my_balance(buy.token.as_deref()).await else { return refuse("the balance could not be read") };
            let need = buy.amount.saturating_add(buy.amount / 20).saturating_add(self.promised_out(network, buy.token.as_deref()).await);
            if have < need {
                return refuse("too little of the coin to lock");
            }
        }
        true
    }

    /// Promises the sale of `buy`, on `network`, once this node has bid on
    /// the buy (`can_take_buy` said yes just before). Nothing for a buy
    /// with no tunnel memo, or one promised already.
    pub async fn promise_buy(&self, network: &str, buy: &Swap) {
        let Memo::Tunnel(from, token_in, amount_in) = parse_memo(&buy.memo) else { return };
        let _one_at_a_time = self.considering.lock().await;
        if self.has(network, buy.id) {
            return;
        }
        self.promise(Promise {
            from: from.clone(),
            token_in,
            amount_in,
            sats: buy.sats,
            to: network.to_string(),
            token: buy.token.clone(),
            amount_out: buy.amount,
            buy_id: buy.id,
            until: Instant::now() + PROMISE_TIME,
            sell_id: None,
        });
        info!(network, swap = buy.id, from = %from, sats = buy.sats, "tunnel: the buy's sale is promised");
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
        Tunnels { networks, settings, key, book, wallet }
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
        // Whether this operator could take the tunnel now: the BTC for its
        // sale on top of the tunnels still live, and the coin to lock. The
        // app refuses to go on when not, before the user pays a fee.
        let btc = self.wallet.can_pay_sats(quote.sats.saturating_add(self.book.promised_sats().await)).await.unwrap_or(false);
        let to_net = &self.networks[to];
        let coin_ok = match to_net.app.my_balance(token).await {
            Ok(have) => have >= quote.amount_out.saturating_add(quote.amount_out / 20).saturating_add(self.book.promised_out(to, token).await),
            Err(_) => false,
        };
        Ok(json!({ "sats": quote.sats, "amountOut": quote.amount_out.to_string(), "canTake": btc && coin_ok }))
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
        Swap { id, side, state, user: "u".into(), token: None, amount, sats, job_id: id, script: script.to_vec(), pay_window: window, memo: vec![] }
    }

    #[tokio::test]
    async fn keeps_its_promise_to_one_sell_on_the_terms_quoted() {
        let s = [0x00, 0x14, 7, 7];
        let w = Some((101, 112));
        let sell = |id, amount, sats, script: &[u8], window| swap(id, Side::Sell, SwapState::Open, amount, sats, script, window);
        let book = TunnelBook::new(100_000);
        let a = Arc::new(Swaps(SyncMutex::new(vec![sell(9, 1_000, 50_000, &s, w), sell(10, 1_000, 50_000, &s, w)])));
        book.add("a", a.clone(), Arc::new(At100), vec![]);
        book.add("b", Arc::new(Swaps(SyncMutex::new(vec![swap(4, Side::Buy, SwapState::Funded, 10, 50_000, &s, None)]))), Arc::new(At100), vec![]);
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
        assert_eq!(parse_memo(b"ipow-tunnel/1 solana-devnet native 60000000"), Memo::Tunnel("solana-devnet".into(), None, 60_000_000));
        assert_eq!(parse_memo(b"ipow-tunnel/1 a 0xAb 7"), Memo::Tunnel("a".into(), Some("0xAb".into()), 7));
        assert_eq!(parse_memo(b""), Memo::None);
        assert_eq!(parse_memo(b"hello"), Memo::None);
        // Claims a tunnel but cannot be read: another version, an extra word,
        // a sign, a number too large, two spaces.
        assert_eq!(parse_memo(b"ipow-tunnel/2 a native 7"), Memo::Unreadable);
        assert_eq!(parse_memo(b"ipow-tunnel/1 a native 7 extra"), Memo::Unreadable);
        assert_eq!(parse_memo(b"ipow-tunnel/1 a native +7"), Memo::Unreadable);
        assert_eq!(parse_memo(b"ipow-tunnel/1 a native 99999999999999999999999999999999999999999"), Memo::Unreadable);
        assert_eq!(parse_memo(b"ipow-tunnel/1 a  native 7"), Memo::Unreadable);
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
