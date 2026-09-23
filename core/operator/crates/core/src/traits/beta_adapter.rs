//! Beta's own trait, parallel to `ApprovingAdapter`/`ConvertingAdapter` but
//! in Beta's vocabulary (`Pending`/`Composition`/`Party`), not Conversion's
//! (`Conversion`/`tx_id: U256`). See DESIGN_V2.md's Beta engine plan for
//! why this is a separate trait rather than an extension of the existing
//! ones: Beta's `iPoWV1` header source is a different deployed instance
//! than Conversion's, and the two apps' lifecycles don't share a phase
//! model beyond both needing Bitcoin-anchor confirmation + a merkle proof.
//!
//! Header freshness for whichever `iPoWV1` a `BetaAdapter` impl reads from
//! is NOT part of this trait — reuse `StreamingAdapter` as-is, pointed at
//! Beta's own `iPoWV1` deployment, exactly as Conversion's engine already
//! does for its own.

use anyhow::Result;
use async_trait::async_trait;

/// A composition's one component, as registered on the hub
/// (`HubCompositionRegistry.Component`) — network id, token id (opaque,
/// network-specific), and the mint rate.
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub network_id: u64,
    pub token_id: [u8; 32],
    pub amount_per_unit: u128,
    pub is_local: bool,
}

/// One pending mint attempt on a hub (`BetaHub.Pending`, or the
/// Solana-hub equivalent — this trait is EVM-only for now per the
/// decided scope, but the shape is chain-agnostic).
#[derive(Debug, Clone)]
pub struct PendingSummary {
    /// A real 32-byte Solana pubkey on an SVM hub; a zero-padded EVM
    /// address (bytes [12:32]) on an EVM hub — matches the MINT
    /// statement's own wire format exactly (`hubUser`, 32 bytes,
    /// zero-padded address on EVM), rather than narrowing to `[u8; 20]`
    /// and losing real Solana pubkeys.
    pub user: [u8; 32],
    pub nonce: u64,
    pub composition_id: u64,
    pub units: u64,
    pub deadline: i64,
    pub approved: bool,
    /// Empty until an operator has started claiming this pending mint.
    pub queued_by: Option<[u8; 32]>,
    /// One entry per remote component, in composition order; `None`
    /// where that component's MINT anchor hasn't been submitted yet.
    pub remote_lock_id: Vec<u64>,
    pub remote_anchor_txid_le: Vec<Option<[u8; 32]>>,
}

/// A remote leg's `Lock` state on its own `BetaVault` (`LockState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteLockState {
    Pending,
    Final,
    Refunded,
}

/// A submitted `ProcessedAnchor`'s on-chain status, read back after
/// `submit_process_anchor`/`submit_attest` to confirm what actually landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorStatus {
    None,
    Queued,
    Exercised,
    Slashed,
    Skipped,
}

/// Everything a Beta engine needs from one hub deployment (`BetaHub`) to
/// drive the claim → relay → exercise lifecycle. A network that's only a
/// spoke implements `BetaSpokeAdapter` below instead/also.
#[async_trait]
pub trait BetaHubAdapter: Send + Sync {
    /// This hub's own `iPoWV1` header source address — NOT necessarily the
    /// same deployment Conversion's engine points at on this chain.
    fn ipow_headers_address(&self) -> String;

    async fn get_composition(
        &self,
        composition_id: u64,
    ) -> Result<Vec<ComponentInfo>>;

    /// `Pending`s that are `approved` but have no live claiming operator
    /// yet — candidates for this operator to claim by building the MINT
    /// statement chain.
    async fn find_claimable_pending(&self) -> Result<Vec<PendingSummary>>;

    async fn get_pending(
        &self,
        user: [u8; 32],
        nonce: u64,
    ) -> Result<PendingSummary>;

    /// Submits a MINT or ATTEST anchor to the hub
    /// (`processAnchor(partyId, statement, txRaw, blockHeight, branchLE,
    /// index)`). Returns the submitted tx hash.
    async fn submit_process_anchor(
        &self,
        party_id: [u8; 32],
        statement: Vec<u8>,
        tx_raw: Vec<u8>,
        block_height: u64,
        branch_le: Vec<[u8; 32]>,
        index: u64,
    ) -> Result<String>;

    async fn get_anchor_status(
        &self,
        txid_le: [u8; 32],
    ) -> Result<AnchorStatus>;

    async fn is_attested(&self, txid_le: [u8; 32]) -> Result<bool>;

    /// `Pending`s whose every remote component is `Queued`, unheld, and
    /// either attested or past its challenge window — ready to exercise.
    async fn find_exercisable_pending(&self) -> Result<Vec<PendingSummary>>;

    async fn exercise_mint(
        &self,
        user: [u8; 32],
        nonce: u64,
    ) -> Result<String>;

    /// This operator's currently registered Beta identity
    /// (`Party.exists`/`dead`/bond), used to decide whether it's even
    /// eligible to claim new pending mints right now.
    async fn get_own_party_status(
        &self,
        party_id: [u8; 32],
    ) -> Result<PartyStatus>;
}

/// Everything a Beta engine needs from one spoke deployment (`BetaVault`)
/// to finalize a remote leg once the hub-side statement chain is anchored.
#[async_trait]
pub trait BetaSpokeAdapter: Send + Sync {
    fn ipow_headers_address(&self) -> String;

    /// Same call shape as the hub's `processAnchor` — the spoke judges
    /// the identical statement independently against its own `Lock`.
    async fn submit_process_anchor(
        &self,
        party_id: [u8; 32],
        statement: Vec<u8>,
        tx_raw: Vec<u8>,
        block_height: u64,
        branch_le: Vec<[u8; 32]>,
        index: u64,
    ) -> Result<String>;

    async fn get_lock_state(&self, lock_id: u64) -> Result<RemoteLockState>;

    async fn get_own_party_status(
        &self,
        party_id: [u8; 32],
    ) -> Result<PartyStatus>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyStatus {
    pub exists: bool,
    pub dead: bool,
    pub bond: u128,
}
