//! What the node needs from the protocol's vault on a network (spec,
//! section 11): the records of the pair Ethereum and Solana, the state the
//! vault keeps, and the calls the operator and guardian make. The same on
//! every network; each network's adapter implements it.

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::types::{Amount, BlockRef};

/// The networks of the pair, as records name them (section 11.5).
pub const ETHEREUM: u8 = 1;
pub const SOLANA: u8 = 2;

/// A record, as section 11.5 writes it. Amounts and fees in gwei.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Record {
    /// A lock on Ethereum, for vETH to `recipient` on Solana.
    Lock { id: u64, amount: u64, recipient: [u8; 32], fee: u64 },
    /// A burn of vETH on Solana, for ETH to `to` on Ethereum.
    Request { id: u64, amount: u64, to: [u8; 20], fee: u64 },
    /// Solana gave up lock `id`: Ethereum returns it.
    Cancel { id: u64 },
    /// A chain's bond on `network`.
    Bond { network: u8, amount: u64 },
    Exit,
}

impl Record {
    /// The network that holds the record's fact and judges it (D109).
    pub fn home(&self) -> Option<u8> {
        match self {
            Record::Lock { .. } => Some(ETHEREUM),
            Record::Request { .. } | Record::Cancel { .. } => Some(SOLANA),
            Record::Bond { network, .. } => Some(*network),
            Record::Exit => None,
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Record::Lock { id, amount, recipient, fee } => {
                out.push(1);
                out.extend_from_slice(&id.to_be_bytes());
                out.extend_from_slice(&amount.to_be_bytes());
                out.extend_from_slice(recipient);
                out.extend_from_slice(&fee.to_be_bytes());
            }
            Record::Request { id, amount, to, fee } => {
                out.push(2);
                out.extend_from_slice(&id.to_be_bytes());
                out.extend_from_slice(&amount.to_be_bytes());
                out.extend_from_slice(to);
                out.extend_from_slice(&fee.to_be_bytes());
            }
            Record::Cancel { id } => {
                out.push(3);
                out.extend_from_slice(&id.to_be_bytes());
            }
            Record::Bond { network, amount } => {
                out.push(4);
                out.push(*network);
                out.extend_from_slice(&amount.to_be_bytes());
            }
            Record::Exit => out.push(5),
        }
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

/// The records of a batch, or `None` when it does not parse, which the
/// vaults judge false.
pub fn decode(batch: &[u8]) -> Option<Vec<Record>> {
    let mut out = vec![];
    let mut o = 0;
    while o < batch.len() {
        let need = match batch[o] {
            1 => 57,
            2 => 45,
            3 => 9,
            4 => 10,
            5 => 1,
            _ => return None,
        };
        if o + need > batch.len() {
            return None;
        }
        let r = match batch[o] {
            1 => Record::Lock {
                id: u64_at(batch, o + 1),
                amount: u64_at(batch, o + 9),
                recipient: batch[o + 17..o + 49].try_into().unwrap(),
                fee: u64_at(batch, o + 49),
            },
            2 => Record::Request {
                id: u64_at(batch, o + 1),
                amount: u64_at(batch, o + 9),
                to: batch[o + 17..o + 37].try_into().unwrap(),
                fee: u64_at(batch, o + 37),
            },
            3 => Record::Cancel { id: u64_at(batch, o + 1) },
            4 => Record::Bond { network: batch[o + 1], amount: u64_at(batch, o + 2) },
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
    /// The bond held here, in this vault's unit: wei on Ethereum, gwei of
    /// vETH on Solana.
    pub bond: u128,
    pub stated: u128,
    /// The other network's bond, counted here once its BOND claim is
    /// official, in gwei.
    pub peer_bond: u64,
    pub peer_bond_carried: bool,
    /// The value of open claims here, in gwei.
    pub open_value: u64,
    pub open_claims: u32,
    /// Money for claim deposits, in the network's coin.
    pub deposits: Amount,
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

/// A lock in the vault on Ethereum. Amounts in gwei.
#[derive(Clone, Debug)]
pub struct Lock {
    pub id: u64,
    pub amount: u64,
    pub recipient: [u8; 32],
    pub fee: u64,
    pub fee_paid: bool,
    pub returned: bool,
}

/// A burn request in the vault on Solana. Amounts in gwei.
#[derive(Clone, Debug)]
pub struct Request {
    pub id: u64,
    pub amount: u64,
    pub to: [u8; 20],
    pub fee: u64,
    pub fee_paid: bool,
}

#[async_trait]
pub trait VaultApp: Send + Sync {
    /// ETHEREUM or SOLANA.
    fn network_id(&self) -> u8;
    /// This node's address here, as the vault names an operator.
    fn me(&self) -> String;
    /// This node's address here, as the other vault's records name it: 20
    /// bytes on Ethereum, 32 on Solana.
    fn me_bytes(&self) -> Vec<u8>;
    /// The flat deposit to object or answer (V1), in the network's coin.
    async fn deposit(&self) -> anyhow::Result<Amount>;

    // The pair chain and the bond (D106, D110)
    /// What this node's registration carries, naming `peer`, its address on
    /// the other network. Registrations name the Ethereum side first, so
    /// both vaults compute the same commitment.
    async fn pair_commitment(&self, peer: &[u8]) -> anyhow::Result<[u8; 32]>;
    async fn chain(&self, operator: &str) -> anyhow::Result<Option<Chain>>;
    async fn register_chain(&self, peer: &[u8], tx: &TxProof, coin_index: u32, tag_index: u32) -> anyhow::Result<()>;
    /// Ethereum: ETH in wei. Solana: vETH in gwei, from this node's account.
    async fn add_bond(&self, amount: u128) -> anyhow::Result<()>;
    async fn add_deposits(&self, amount: Amount) -> anyhow::Result<()>;

    // Real Bitcoin (D108, D116, D118)
    async fn is_real(&self, block: &BlockRef) -> anyhow::Result<bool>;
    /// The least escrow of a job whose proof block counts as real (D118).
    async fn min_certifying_escrow(&self) -> anyhow::Result<Amount>;
    async fn record_real_from_job(&self, job_id: u64) -> anyhow::Result<()>;
    /// `low` is below `high`, which is real.
    async fn record_real(&self, low: &BlockRef, high: &BlockRef) -> anyhow::Result<()>;
    async fn open_checkpoint(&self, confirmations: u16, paid: Amount) -> anyhow::Result<u64>;

    // Messages (D107)
    /// Submits the next message of `operator`'s chain.
    async fn submit_message(&self, operator: &str, tx: &TxProof, input_index: u32, tag_index: u32, batch: &[u8]) -> anyhow::Result<()>;

    // The facts this vault holds
    /// Ethereum: locks with an id above `after` whose Ethereum block is
    /// final, oldest first. Empty on Solana.
    async fn final_locks_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Lock>>;
    async fn lock(&self, id: u64) -> anyhow::Result<Option<Lock>>;
    /// Solana: requests with an id above `after`, oldest first. Empty on
    /// Ethereum.
    async fn requests_after(&self, after: u64, limit: usize) -> anyhow::Result<Vec<Request>>;
    async fn request(&self, id: u64) -> anyhow::Result<Option<Request>>;
    /// Solana: whether lock `id`'s recipient gave it up.
    async fn given_up(&self, id: u64) -> anyhow::Result<bool>;
    /// Solana: whether lock `id`'s receipt was issued.
    async fn receipt_issued(&self, id: u64) -> anyhow::Result<bool>;
    /// Ethereum: whether request `id` was paid.
    async fn request_paid(&self, id: u64) -> anyhow::Result<bool>;

    // Claims (D111)
    async fn claim_count(&self) -> anyhow::Result<u64>;
    /// A claim with the acting records it carries.
    async fn claim(&self, id: u64) -> anyhow::Result<Claim>;
    /// A claim without its records: all an operator needs to answer,
    /// decide and collect, read without event logs.
    async fn claim_status(&self, id: u64) -> anyhow::Result<Claim>;
    async fn object(&self, id: u64) -> anyhow::Result<()>;
    async fn answer(&self, id: u64) -> anyhow::Result<()>;
    async fn decide(&self, id: u64) -> anyhow::Result<()>;
    /// Collects this node's winning deposits of a decided claim, if any.
    async fn collect(&self, id: u64) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_round_trip_in_the_bytes_of_section_11_5() {
        let records = vec![
            Record::Lock { id: 99, amount: 1_000_000_000, recipient: [7; 32], fee: 5 },
            Record::Request { id: 7, amount: 2, to: [9; 20], fee: 0 },
            Record::Cancel { id: 99 },
            Record::Bond { network: ETHEREUM, amount: 10 },
            Record::Exit,
        ];
        let batch = encode(&records);
        assert_eq!(batch.len(), 57 + 45 + 9 + 10 + 1);
        assert_eq!(&batch[..9], &[1, 0, 0, 0, 0, 0, 0, 0, 99]);
        assert_eq!(decode(&batch).unwrap(), records);
        // EXIT must be last; unknown kinds and short records do not parse.
        assert!(decode(&encode(&[Record::Exit, Record::Cancel { id: 1 }])).is_none());
        assert!(decode(&[9]).is_none());
        assert!(decode(&batch[..56]).is_none());
    }
}
