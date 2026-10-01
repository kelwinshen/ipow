//! What the node needs from the protocol's vault on a network (spec,
//! section 11): the records of the pair Ethereum and Solana, the state the
//! vault keeps, and the calls the operator and guardian make. Each network
//! is the home of its own assets and holds receipts of the other's (section
//! 11.9). The same on every network; each network's adapter implements it.

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::types::{Amount, BlockRef};

/// Network numbers, fixed for good (D133). A vault serves one pair of them
/// (D132).
pub const ETHEREUM: u8 = 1;
pub const SOLANA: u8 = 2;
pub const BASE: u8 = 3;
pub const ROBINHOOD: u8 = 4;
pub const POLKADOT: u8 = 5;
pub const HEDERA: u8 = 6;
pub const HYPERLIQUID: u8 = 7;
pub const TEMPO: u8 = 8;
pub const MAX_NETWORK: u8 = 8;

/// D119: a message is false when its batch is longer than this, its Bitcoin
/// transaction longer than this, or it carries more LOCK, REQUEST, CANCEL
/// and ASSET records than this, on both networks together.
pub const MAX_BATCH: usize = 2048;
pub const MAX_RAW_TX: usize = 1024;
pub const MAX_RECORDS: usize = 32;
/// A message acts on at most this many LOCK records on Solana.
pub const MAX_LOCKS_ON_SOLANA: usize = 10;
/// The assets a chain keeps bonds in, and a claim moves, at most.
pub const MAX_ASSETS: usize = 8;

/// A record, as section 11.9 writes it. Amounts and fees in the receipt's
/// smallest unit; addresses in 32 bytes, an Ethereum address in the last 20.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Record {
    /// A lock of asset `asset` of network `home`, for its receipt on the
    /// other network to `recipient`. `at`: the lock's time, in seconds.
    Lock { home: u8, asset: u32, id: u64, amount: u64, recipient: [u8; 32], fee: u64, fast_fee: u64, at: u64 },
    /// A burn on network `net` of a receipt of the other network's asset
    /// `asset`, for the asset to `to` there. `at`: the burn's time.
    Request { net: u8, asset: u32, id: u64, amount: u64, to: [u8; 32], fee: u64, fast_fee: u64, at: u64 },
    /// Network `net` gave up lock `id` of the other network: it returns it.
    Cancel { net: u8, id: u64 },
    /// A chain's bond on network `net` in asset `asset` of network `home`.
    Bond { net: u8, home: u8, asset: u32, amount: u64 },
    /// Asset `asset` of network `home`: its token, and its receipt's
    /// decimals.
    Asset { home: u8, asset: u32, token: [u8; 32], decimals: u8 },
    Exit,
}

impl Record {
    /// The network that holds the record's fact and judges it (D109).
    pub fn fact(&self) -> Option<u8> {
        match self {
            Record::Lock { home, .. } | Record::Asset { home, .. } => Some(*home),
            Record::Request { net, .. } | Record::Cancel { net, .. } | Record::Bond { net, .. } => Some(*net),
            Record::Exit => None,
        }
    }

    /// Whether Solana reads an account for it, or the record counts toward
    /// D119's 32.
    pub fn counted(&self) -> bool {
        matches!(self, Record::Lock { .. } | Record::Request { .. } | Record::Cancel { .. } | Record::Asset { .. })
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Record::Lock { home, asset, id, amount, recipient, fee, fast_fee, at } => {
                out.extend_from_slice(&[1, *home]);
                out.extend_from_slice(&asset.to_be_bytes());
                out.extend_from_slice(&id.to_be_bytes());
                out.extend_from_slice(&amount.to_be_bytes());
                out.extend_from_slice(recipient);
                out.extend_from_slice(&fee.to_be_bytes());
                out.extend_from_slice(&fast_fee.to_be_bytes());
                out.extend_from_slice(&at.to_be_bytes());
            }
            Record::Request { net, asset, id, amount, to, fee, fast_fee, at } => {
                out.extend_from_slice(&[2, *net]);
                out.extend_from_slice(&asset.to_be_bytes());
                out.extend_from_slice(&id.to_be_bytes());
                out.extend_from_slice(&amount.to_be_bytes());
                out.extend_from_slice(to);
                out.extend_from_slice(&fee.to_be_bytes());
                out.extend_from_slice(&fast_fee.to_be_bytes());
                out.extend_from_slice(&at.to_be_bytes());
            }
            Record::Cancel { net, id } => {
                out.extend_from_slice(&[3, *net]);
                out.extend_from_slice(&id.to_be_bytes());
            }
            Record::Bond { net, home, asset, amount } => {
                out.extend_from_slice(&[4, *net, *home]);
                out.extend_from_slice(&asset.to_be_bytes());
                out.extend_from_slice(&amount.to_be_bytes());
            }
            Record::Asset { home, asset, token, decimals } => {
                out.extend_from_slice(&[6, *home]);
                out.extend_from_slice(&asset.to_be_bytes());
                out.extend_from_slice(token);
                out.push(*decimals);
            }
            Record::Exit => out.push(5),
        }
    }

    /// The record's bytes.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out = vec![];
        self.write(&mut out);
        out
    }
}

/// A batch: its records one after the other (D114). EXIT, if any, must be
/// last; `encode` does not check it.
pub fn encode(records: &[Record]) -> Vec<u8> {
    let mut out = vec![];
    for r in records {
        r.write(&mut out);
    }
    out
}

fn u64_at(b: &[u8], o: usize) -> u64 {
    u64::from_be_bytes(b[o..o + 8].try_into().unwrap())
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes(b[o..o + 4].try_into().unwrap())
}

/// The records of a batch, or `None` when it does not parse, which the
/// vaults judge false.
pub fn decode(batch: &[u8]) -> Option<Vec<Record>> {
    let mut out = vec![];
    let mut o = 0;
    while o < batch.len() {
        let need = match batch[o] {
            1 | 2 => 78,
            3 => 10,
            4 => 15,
            5 => 1,
            6 => 39,
            _ => return None,
        };
        if o + need > batch.len() {
            return None;
        }
        let b = &batch[o..o + need];
        let r = match b[0] {
            1 => Record::Lock {
                home: b[1],
                asset: u32_at(b, 2),
                id: u64_at(b, 6),
                amount: u64_at(b, 14),
                recipient: b[22..54].try_into().unwrap(),
                fee: u64_at(b, 54),
                fast_fee: u64_at(b, 62),
                at: u64_at(b, 70),
            },
            2 => Record::Request {
                net: b[1],
                asset: u32_at(b, 2),
                id: u64_at(b, 6),
                amount: u64_at(b, 14),
                to: b[22..54].try_into().unwrap(),
                fee: u64_at(b, 54),
                fast_fee: u64_at(b, 62),
                at: u64_at(b, 70),
            },
            3 => Record::Cancel { net: b[1], id: u64_at(b, 2) },
            4 => Record::Bond { net: b[1], home: b[2], asset: u32_at(b, 3), amount: u64_at(b, 7) },
            6 => Record::Asset { home: b[1], asset: u32_at(b, 2), token: b[6..38].try_into().unwrap(), decimals: b[38] },
            _ => {
                if o + 1 != batch.len() {
                    return None;
                }
                Record::Exit
            }
        };
        out.push(r);
        o += need;
    }
    Some(out)
}

/// What a message carries in its `OP_RETURN` (D107).
pub fn message_payload(batch: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"iPoW vault");
    h.update(batch);
    h.finalize().into()
}

/// An Ethereum address in 32 bytes.
pub fn eth_address32(address: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(address);
    out
}

/// A Bitcoin transaction in a block at or below a block the vault recorded
/// as real (D108).
#[derive(Clone, Debug)]
pub struct TxProof {
    pub block: BlockRef,
    pub raw_tx: Vec<u8>,
    pub siblings: Vec<[u8; 32]>,
    pub tx_index: u64,
    pub real: BlockRef,
}

/// A chain's place in one asset (D129), in record units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Position {
    pub home: u8,
    pub asset: u32,
    /// Held here: the asset itself on its home network, its receipt on the
    /// other.
    pub bond: u64,
    /// The part its BOND record stated.
    pub stated: u64,
    /// Its bond in the asset on the other network, counted here once its
    /// BOND claim is official.
    pub peer_bond: u64,
    pub peer_bond_carried: bool,
    /// Open claims here in the asset.
    pub open_value: u64,
}

/// An operator's pair chain, as one vault sees it.
#[derive(Clone, Debug, Default)]
pub struct Chain {
    /// The same operator on the other network: 32 bytes for Solana, 20 for
    /// Ethereum.
    pub peer: Vec<u8>,
    pub coin: ([u8; 32], u32),
    pub messages: u64,
    pub exited: bool,
    pub slashed: bool,
    pub refused: bool,
    pub positions: Vec<Position>,
    pub open_claims: u32,
    /// Money for claim deposits, in the network's coin.
    pub deposits: Amount,
}

impl Chain {
    /// Its place in asset (`home`, `asset`), empty if none.
    pub fn position(&self, home: u8, asset: u32) -> Position {
        self.positions.iter().find(|p| p.home == home && p.asset == asset).cloned().unwrap_or(Position { home, asset, ..Default::default() })
    }
}

/// A claim: the acting records of one message (D111).
#[derive(Clone, Debug)]
pub struct Claim {
    pub id: u64,
    pub operator: String,
    pub last_at: i64,
    pub held: bool,
    pub decided: bool,
    pub accepted: bool,
    /// The acting records it carries.
    pub records: Vec<Record>,
}

/// An asset whose home is this network (section 11.9).
#[derive(Clone, Debug, PartialEq)]
pub struct AssetInfo {
    pub number: u32,
    /// Its token, in 32 bytes; zero for the network's coin.
    pub token: [u8; 32],
    /// Its receipt's decimals, at most 9.
    pub decimals: u8,
}

impl AssetInfo {
    pub fn record(&self, home: u8) -> Record {
        Record::Asset { home, asset: self.number, token: self.token, decimals: self.decimals }
    }
}

/// A lock of an asset whose home is this network. Record units.
#[derive(Clone, Debug)]
pub struct Lock {
    pub id: u64,
    pub asset: u32,
    pub amount: u64,
    pub recipient: [u8; 32],
    pub fee: u64,
    /// For an attester of the fast path, or the recipient (D122).
    pub fast_fee: u64,
    /// Its time, in seconds.
    pub locked_at: u64,
    pub fee_paid: bool,
    pub returned: bool,
}

impl Lock {
    /// The LOCK record that states this lock, of network `home`.
    pub fn record(&self, home: u8) -> Record {
        Record::Lock {
            home,
            asset: self.asset,
            id: self.id,
            amount: self.amount,
            recipient: self.recipient,
            fee: self.fee,
            fast_fee: self.fast_fee,
            at: self.locked_at,
        }
    }
}

/// A burn on this network of a receipt of the other network's asset.
#[derive(Clone, Debug)]
pub struct Request {
    pub id: u64,
    pub asset: u32,
    pub amount: u64,
    pub to: [u8; 32],
    pub fee: u64,
    /// For an attester who pays it at once, or `to` (D122).
    pub fast_fee: u64,
    /// When it was made, in seconds.
    pub requested_at: u64,
    pub fee_paid: bool,
}

impl Request {
    /// The REQUEST record that states this burn, made on network `net`.
    pub fn record(&self, net: u8) -> Record {
        Record::Request {
            net,
            asset: self.asset,
            id: self.id,
            amount: self.amount,
            to: self.to,
            fee: self.fee,
            fast_fee: self.fast_fee,
            at: self.requested_at,
        }
    }
}

/// The keccak-256 hash of a record's bytes: how Ethereum keeps the record
/// an attest stated.
pub fn record_hash(r: &Record) -> [u8; 32] {
    use sha3::Digest as _;
    sha3::Keccak256::digest(r.bytes()).into()
}

/// An attest of a lock of the other network: a receipt issued at once
/// (section 11.7).
#[derive(Clone, Debug)]
pub struct FastLock {
    /// Its number: attests are counted, so every one can be found.
    pub id: u64,
    /// The attester, as the vault names it.
    pub attester: String,
    /// The lock it is for, and `record_hash` of the LOCK record it stated.
    pub lock_id: u64,
    pub stated: [u8; 32],
    /// The asset of the lock it stated, whose receipts it locked.
    pub asset: u32,
    pub collateral: u64,
    pub attested_at: i64,
    /// The claim linked to it; zero if none.
    pub claim: u64,
    /// The attest of the same lock made before it, settled first; zero if
    /// none (D126).
    pub prev: u64,
    pub burned: bool,
}

#[async_trait]
pub trait VaultApp: Send + Sync {
    /// This network's number (D133).
    fn network_id(&self) -> u8;
    /// The number of the pair's other network.
    fn peer_id(&self) -> u8;
    /// This vault as the other network's vault names it, in 32 bytes: an
    /// EVM vault's address in the last 20, on Solana the pair's
    /// configuration account (section 11.3).
    fn vault_id(&self) -> [u8; 32];
    /// The vault this one names as its pair's other, in the same form.
    async fn peer_vault(&self) -> anyhow::Result<[u8; 32]>;
    /// This node's address here, as the vault names an operator.
    fn me(&self) -> String;
    /// This node's address here, as the other vault's records name it: 20
    /// bytes on Ethereum, 32 on Solana.
    fn me_bytes(&self) -> Vec<u8>;
    /// The flat deposit to object or answer (D121), in the network's coin.
    async fn deposit(&self) -> anyhow::Result<Amount>;

    // The pair chain and the bonds (D106, D110, D129)
    /// What this node's registration carries, naming `peer`, its address on
    /// the other network.
    async fn pair_commitment(&self, peer: &[u8]) -> anyhow::Result<[u8; 32]>;
    async fn chain(&self, operator: &str) -> anyhow::Result<Option<Chain>>;
    /// A chain's place in asset (`home`, `asset`), also one it holds no bond
    /// in: what its claims move there and the bond the other network counts.
    async fn position(&self, operator: &str, home: u8, asset: u32) -> anyhow::Result<Position>;
    async fn register_chain(&self, peer: &[u8], tx: &TxProof, coin_index: u32, tag_index: u32) -> anyhow::Result<()>;
    /// Adds to this node's bond in asset (`home`, `asset`), in record units:
    /// the asset on its home network, its receipt on the other.
    async fn add_bond(&self, home: u8, asset: u32, amount: u64) -> anyhow::Result<()>;
    async fn add_deposits(&self, amount: Amount) -> anyhow::Result<()>;
    /// Moves a slashed chain's backing in an asset into place (D109). On
    /// Solana it also credits the submitter's 20%.
    async fn settle_slash(&self, operator: &str, home: u8, asset: u32) -> anyhow::Result<()>;
    /// What of a slashed chain's bond in an asset is not settled yet.
    async fn slash_pending(&self, operator: &str, home: u8, asset: u32) -> anyhow::Result<u64>;

    // Real Bitcoin (D108, D116, D118)
    async fn is_real(&self, block: &BlockRef) -> anyhow::Result<bool>;
    /// The least escrow of a job whose proof block counts as real (D118).
    async fn min_certifying_escrow(&self) -> anyhow::Result<Amount>;
    async fn record_real_from_job(&self, job_id: u64) -> anyhow::Result<()>;
    /// `low` is below `high`, which is real.
    async fn record_real(&self, low: &BlockRef, high: &BlockRef) -> anyhow::Result<()>;
    async fn open_checkpoint(&self, confirmations: u16, paid: Amount) -> anyhow::Result<u64>;

    // Messages (D107)
    /// The batch of message `index` of `operator`'s chain, as this vault
    /// published it when it processed the message; `None` when it has not.
    async fn message_batch(&self, operator: &str, index: u64) -> anyhow::Result<Option<Vec<u8>>>;
    /// Submits the next message of `operator`'s chain.
    async fn submit_message(&self, operator: &str, tx: &TxProof, input_index: u32, tag_index: u32, batch: &[u8]) -> anyhow::Result<()>;

    // Assets whose home is this network, and their locks
    /// The assets registered here, the network's coin first.
    async fn assets(&self) -> anyhow::Result<Vec<AssetInfo>>;
    /// Locks with an id above `after` that can be carried: their block is
    /// final, oldest first.
    async fn final_locks_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Lock>>;
    async fn lock(&self, id: u64) -> anyhow::Result<Option<Lock>>;
    /// Takes the fee this node earned for the first true message carrying
    /// `record`, a lock or a burn made here, when the vault leaves it to be
    /// taken (Solana, section 11.9). Elsewhere it is already in this node's
    /// credit. Does nothing when there is none to take.
    async fn take_fee(&self, record: &Record) -> anyhow::Result<()>;
    /// Whether the other network's burn `id`, of a receipt of an asset here,
    /// was paid here.
    async fn request_paid(&self, id: u64) -> anyhow::Result<bool>;

    // Receipts here of the other network's assets
    /// Whether the receipt of the other network's asset `asset` exists here.
    async fn has_receipt(&self, asset: u32) -> anyhow::Result<bool>;
    /// Burns of receipts with an id above `after` that can be carried: final,
    /// oldest first.
    async fn requests_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Request>>;
    async fn request(&self, id: u64) -> anyhow::Result<Option<Request>>;
    /// Makes the receipt of the other network's asset from accepted claim
    /// `claim` carrying its ASSET record. Anyone may.
    async fn make_receipt(&self, claim: u64, record: &Record) -> anyhow::Result<()>;
    /// What this node holds of the receipt of the other network's asset
    /// `asset`, in record units.
    async fn receipt_balance(&self, asset: u32) -> anyhow::Result<u64>;
    /// Whether the other network's lock `id` was given up here, as of a final
    /// state.
    async fn given_up(&self, id: u64) -> anyhow::Result<bool>;
    /// Whether the other network's lock `id` had its receipt counted here.
    async fn receipt_issued(&self, id: u64) -> anyhow::Result<bool>;
    /// The open attests of the other network's lock `id`, newest first, and
    /// when its first attest was made; `None` when it was never attested.
    async fn lock_attests(&self, id: u64) -> anyhow::Result<Option<(i64, Vec<FastLock>)>>;

    // Claims (D111)
    async fn claim_count(&self) -> anyhow::Result<u64>;
    /// A claim with the acting records it carries.
    async fn claim(&self, id: u64) -> anyhow::Result<Claim>;
    /// A claim without its records, read without event logs.
    async fn claim_status(&self, id: u64) -> anyhow::Result<Claim>;
    async fn object(&self, id: u64) -> anyhow::Result<()>;
    async fn answer(&self, id: u64) -> anyhow::Result<()>;
    async fn decide(&self, id: u64) -> anyhow::Result<()>;
    /// Collects this node's winning deposits of a decided claim, if any.
    async fn collect(&self, id: u64) -> anyhow::Result<()>;

    // The fast paths (section 11.7)
    /// Issues the receipt of the other network's lock `record` at once,
    /// locking 1.25 times its amount of this node's receipts. Returns the
    /// attest's number.
    async fn attest_lock(&self, record: &Record) -> anyhow::Result<u64>;
    async fn attest_count(&self) -> anyhow::Result<u64>;
    async fn fast_lock(&self, id: u64) -> anyhow::Result<Option<FastLock>>;
    /// Links attest `attest` to claim `claim` of this node's chain;
    /// `linked` is the claim linked before.
    async fn link_fast(&self, claim: u64, attest: u64, linked: Option<u64>) -> anyhow::Result<()>;
    async fn burn_fast(&self, attest: u64, linked: Option<u64>) -> anyhow::Result<()>;
    async fn settle_fast(&self, claim: u64, attest: u64) -> anyhow::Result<()>;
    /// Pays the other network's burn `record`, of a receipt of an asset here,
    /// at once from this node's money.
    async fn fast_pay(&self, record: &Record) -> anyhow::Result<()>;
    /// Who paid burn `record` at once under it, if anyone.
    async fn fast_paid_by(&self, record: &Record) -> anyhow::Result<Option<String>>;
    /// Pays a burn carried by accepted claim `claim`, to whoever it is owed.
    async fn pay_request(&self, claim: u64, record: &Record) -> anyhow::Result<()>;

    // Credits: won deposits, a slasher's share, settled attests' collateral
    /// This node's credit here in asset (`home`, `asset`), in the network's
    /// smallest unit of it: the asset on its home, its receipt on the other.
    async fn credit(&self, home: u8, asset: u32) -> anyhow::Result<u128>;
    /// Withdraws it to this node's own account.
    async fn withdraw_credit(&self, home: u8, asset: u32) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_round_trip_in_the_bytes_of_section_11_9() {
        let records = vec![
            Record::Lock { home: ETHEREUM, asset: 0, id: 99, amount: 1_000_000_000, recipient: [7; 32], fee: 5, fast_fee: 6, at: 1_800_000_000 },
            Record::Request { net: SOLANA, asset: 2, id: 7, amount: 2, to: eth_address32(&[9; 20]), fee: 0, fast_fee: 3, at: 1_800_000_001 },
            Record::Cancel { net: SOLANA, id: 99 },
            Record::Bond { net: ETHEREUM, home: SOLANA, asset: 1, amount: 10 },
            Record::Asset { home: SOLANA, asset: 3, token: [4; 32], decimals: 6 },
            Record::Exit,
        ];
        let batch = encode(&records);
        assert_eq!(batch.len(), 78 + 78 + 10 + 15 + 39 + 1);
        assert_eq!(&batch[..6], &[1, 1, 0, 0, 0, 0]);
        assert_eq!(decode(&batch).unwrap(), records);
        // EXIT must be last; unknown kinds and short records do not parse.
        assert!(decode(&encode(&[Record::Exit, Record::Cancel { net: 1, id: 1 }])).is_none());
        assert!(decode(&[9]).is_none());
        assert!(decode(&batch[..77]).is_none());
    }
}
