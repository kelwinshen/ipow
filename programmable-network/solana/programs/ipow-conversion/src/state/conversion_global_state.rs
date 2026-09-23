use anchor_lang::prelude::*;

/// This program's own minimal global state — independent of `ipow`'s
/// `GlobalState`, matching `ipow-message-relay`'s `RelayGlobalState`
/// pattern (own counter, own governance, no shared mutable state across
/// programs). `governance` only gates network config
/// (`add_network`/`update_network`) and liquidity provisioning — never the
/// claiming role, which is the whole point of this redesign.
#[account]
#[derive(InitSpace)]
pub struct ConversionGlobalState {
    pub governance: Pubkey,
    pub next_tx_id: u64,
    pub commit_fee_bps: u16,
}
