use anchor_lang::prelude::*;

use crate::constants::{MAX_ASSETS, MAX_BATCH, MAX_RAW_TX};

/// The vault's one settings record. Its address is the key the vault
/// registered with the protocol, the authority of every receipt and of the
/// vault's token accounts, and the holder of SOL: deposits, locked SOL and
/// SOL bonds.
#[account]
#[derive(InitSpace)]
pub struct Config {
    /// The pair's other network (D132, D133), and its vault there, as a
    /// registration names it (D106).
    pub peer: u8,
    pub peer_vault: [u8; 20],
    /// D121: the flat deposit to object or answer, and the operator's deposit
    /// for a claim, in lamports.
    pub deposit: u64,
    /// D118: the least escrow of a job whose proof block counts as real, in
    /// lamports.
    pub min_certifying_escrow: u64,
    pub claim_count: u64,
    pub request_count: u64,
    pub checkpoint_count: u64,
    pub attest_count: u64,
    /// Assets whose home is Solana, SOL being 0 (D128).
    pub asset_count: u32,
    /// Locks of them.
    pub lock_count: u64,
    pub bump: u8,
    /// Genesis (docs/specs/ipow-vault-genesis.md, D142 to D147): the key
    /// that may make receipts and issue named locks until it finalizes or
    /// `genesis_end`; the default key when the pair has no genesis.
    pub genesis_key: Pubkey,
    pub genesis_end: i64,
    pub genesis_done: bool,
}

/// An asset whose home is Solana (section 11.9): SOL, number 0, or a token
/// anyone registered.
#[account]
#[derive(InitSpace)]
pub struct HomeAsset {
    pub number: u32,
    /// The default key for SOL.
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub decimals: u8,
    /// The receipt's decimals, at most 9: records count in its unit.
    pub record_decimals: u8,
    /// 10 ** (decimals - record_decimals): native units per record unit.
    pub unit: u64,
    /// What backs its receipts on Ethereum, in record units: the locks not
    /// returned, less what was paid out, plus the backing share of slashes.
    pub reserve: u64,
    pub bump: u8,
}

/// A chain's place in one asset (D129), in record units.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct Position {
    pub home: u8,
    pub asset: u32,
    /// Held here: the asset itself when its home is Solana, its receipt
    /// otherwise.
    pub bond: u64,
    /// The part its BOND record stated.
    pub stated: u64,
    /// Its bond in the asset on Ethereum, counted once its BOND claim is
    /// official.
    pub peer_bond: u64,
    pub peer_bond_carried: bool,
    /// Open claims here in the asset.
    pub open_value: u64,
    /// After a slash: the backing's 80%, and the submitter's 20%, not
    /// settled yet.
    pub slash_backing: u64,
    pub slash_share: u64,
}

/// An operator's pair chain (D106) and what it keeps in the vault.
#[account]
#[derive(InitSpace)]
pub struct Chain {
    pub operator: Pubkey,
    /// The same operator on Ethereum.
    pub peer_operator: [u8; 20],
    pub coin_txid: [u8; 32],
    pub coin_vout: u32,
    pub exited: bool,
    /// A false record was proven here: every bond is gone (D109).
    pub slashed: bool,
    /// Who submitted the message proven false: it takes the 20%.
    pub slasher: Pubkey,
    /// A claim of the chain was refused here: no other claim acts (D111).
    pub refused: bool,
    pub messages: u64,
    /// Lamports for the flat deposits of its claims.
    pub deposits: u64,
    pub open_claims: u32,
    #[max_len(MAX_ASSETS)]
    pub positions: Vec<Position>,
    pub bump: u8,
}

impl Chain {
    pub fn position(&self, home: u8, asset: u32) -> Option<usize> {
        self.positions.iter().position(|p| p.home == home && p.asset == asset)
    }

    /// The chain's position in an asset, made on first use, at most
    /// `MAX_ASSETS`.
    pub fn slot(&mut self, home: u8, asset: u32) -> Result<usize> {
        if let Some(i) = self.position(home, asset) {
            return Ok(i);
        }
        require!(self.positions.len() < MAX_ASSETS, crate::errors::VaultError::TooManyAssets);
        self.positions.push(Position { home, asset, ..Default::default() });
        Ok(self.positions.len() - 1)
    }
}

/// What a claim moves in one asset, and a BOND it carries.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct ClaimAsset {
    pub home: u8,
    pub asset: u32,
    pub value: u64,
    pub peer_bond: u64,
}

/// The acting records of one message (D111).
#[account]
pub struct Claim {
    pub id: u64,
    pub operator: Pubkey,
    pub last_at: i64,
    /// When it opened: an attest waits only for a claim opened in time.
    pub opened_at: i64,
    pub held: bool,
    pub decided: bool,
    pub accepted: bool,
    pub answers: u32,
    pub objections: u32,
    /// Set when decided: what each deposit of the winning side collects.
    pub payout: u64,
    pub assets: Vec<ClaimAsset>,
    /// The records from Ethereum it acts on, one after the other.
    pub records: Vec<u8>,
    pub bump: u8,
}

impl Claim {
    pub fn space(assets: usize, records: usize) -> usize {
        8 + 8 + 32 + 8 + 8 + 3 + 4 + 4 + 8 + 4 + assets * (1 + 4 + 8 + 8) + 4 + records + 1
    }

    /// Whether it acts on exactly `record`.
    pub fn carries(&self, record: &[u8]) -> bool {
        let mut o = 0;
        while o < self.records.len() {
            let len = crate::util::record_len(self.records[o]);
            if len == 0 || o + len > self.records.len() {
                return false;
            }
            if &self.records[o..o + len] == record {
                return true;
            }
            o += len;
        }
        false
    }
}

/// The deposits one address put down on each side of one claim.
#[account]
#[derive(InitSpace)]
pub struct Stake {
    pub answers: u32,
    pub objections: u32,
    pub bump: u8,
}

/// A block known to be on real Bitcoin (D108).
#[account]
#[derive(InitSpace)]
pub struct Real {
    pub bump: u8,
}

/// A lock here of an asset whose home is Solana, for its receipt on
/// Ethereum (section 11.9).
#[account]
#[derive(InitSpace)]
pub struct HomeLock {
    pub id: u64,
    pub asset: u32,
    pub owner: Pubkey,
    pub amount: u64,
    /// On Ethereum: an address in the last 20 bytes.
    pub recipient: [u8; 32],
    pub fee: u64,
    /// For the attester of the fast path, or the recipient (D122).
    pub fast_fee: u64,
    pub locked_at: i64,
    /// The operator of the first true message carrying it, which takes the
    /// fee; default until one does.
    pub fee_to: Pubkey,
    pub fee_paid: bool,
    /// The fee was taken; the record keeps stating it.
    pub fee_taken: bool,
    pub returned: bool,
    pub bump: u8,
}

/// A burn here of a receipt, for the asset on Ethereum (section 11.9).
#[account]
#[derive(InitSpace)]
pub struct Request {
    pub id: u64,
    pub asset: u32,
    pub owner: Pubkey,
    pub amount: u64,
    /// On Ethereum: an address in the last 20 bytes.
    pub to: [u8; 32],
    pub fee: u64,
    /// For the attester who pays it at once on Ethereum, or `to` (D122).
    /// Burned with the amount.
    pub fast_fee: u64,
    /// When it was made, in its REQUEST record (D124).
    pub requested_at: i64,
    /// The operator of the first true message carrying it, which takes the
    /// fee; default until one does.
    pub fee_to: Pubkey,
    pub fee_paid: bool,
    /// The fee was taken; the record keeps stating it.
    pub fee_taken: bool,
    pub bump: u8,
}

/// What became of a lock on Ethereum here. It exists once something
/// happened, so each happens once.
#[account]
#[derive(InitSpace)]
pub struct LockMark {
    pub lock_id: u64,
    /// Its receipt counted: issued after its claim, or delivered by an
    /// attest settled with the true record.
    pub issued: bool,
    pub given_up: bool,
    /// Attests of the lock (D126), those settled, when its first was made,
    /// and its latest: each names the one before.
    pub attests: u32,
    pub settled: u32,
    pub first_at: i64,
    pub last_attest: u64,
    pub bump: u8,
}

/// A lock on Ethereum whose receipt an attester issued at once, with the
/// LOCK record it stated and the receipts it locked (section 11.7, D123).
#[account]
#[derive(InitSpace)]
pub struct FastLock {
    /// Its number: guardians find every attest by it, also one naming a lock
    /// that does not exist.
    pub id: u64,
    pub lock_id: u64,
    pub asset: u32,
    pub attester: Pubkey,
    pub amount: u64,
    pub recipient: Pubkey,
    pub fee: u64,
    pub fast_fee: u64,
    pub locked_at: i64,
    /// Receipts locked by the attester, 1.25 times the amount.
    pub collateral: u64,
    pub attested_at: i64,
    /// A claim of the attester's own chain carrying the same record, opened
    /// in time; zero if none (D125).
    pub claim: u64,
    /// The attest of the same lock made before it, settled first (D126).
    pub prev: u64,
    pub burned: bool,
    pub bump: u8,
}

/// A burn on Ethereum of a receipt of an asset here, paid at once by an
/// attester under the record it stated (section 11.7).
#[account]
#[derive(InitSpace)]
pub struct FastPay {
    pub attester: Pubkey,
    pub paid_at: i64,
    pub bump: u8,
}

/// A burn on Ethereum of a receipt of an asset here, paid.
#[account]
#[derive(InitSpace)]
pub struct Paid {
    pub bump: u8,
}

/// What an address can withdraw in one asset: lamports for SOL, token
/// units for a token whose home is Solana, a receipt's units for an asset
/// of Ethereum.
#[account]
#[derive(InitSpace)]
pub struct Credit {
    pub owner: Pubkey,
    pub home: u8,
    pub asset: u32,
    pub amount: u64,
    pub bump: u8,
}

/// A message's Bitcoin transaction and batch, uploaded in pieces when they do
/// not fit in one Solana transaction with its proof.
#[account]
#[derive(InitSpace)]
pub struct Buffer {
    pub owner: Pubkey,
    #[max_len(MAX_RAW_TX)]
    pub raw_tx: Vec<u8>,
    #[max_len(MAX_BATCH)]
    pub batch: Vec<u8>,
    pub bump: u8,
}

/// The batch of one processed message, published so that anyone can bring
/// the message to the other network (D120). Anyone may close it
/// `MESSAGE_KEPT` after it was processed, its rent back to whoever paid it.
#[account]
pub struct Message {
    pub operator: Pubkey,
    pub payer: Pubkey,
    pub index: u64,
    pub processed_at: i64,
    pub batch: Vec<u8>,
    pub bump: u8,
}

impl Message {
    pub fn space(batch_len: usize) -> usize {
        8 + 32 + 32 + 8 + 8 + 4 + batch_len + 1
    }
}

/// A Bitcoin transaction in a block below a real block (D108).
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Btc {
    pub block: ipow_protocol::state::BlockRef,
    pub raw_tx: Vec<u8>,
    pub siblings: Vec<[u8; 32]>,
    pub tx_index: u64,
    /// A block already recorded as real, with `block` below it.
    pub real: ipow_protocol::state::BlockRef,
}
