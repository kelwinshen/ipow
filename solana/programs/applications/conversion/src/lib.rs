//! Conversion: an application on the iPoW protocol (D5, D8). A user swaps
//! SOL or a token for real BTC, or real BTC for them, at a price the user
//! sets; the operator that wins the job is the other side of the swap. The
//! same rules as `contracts/applications/conversion/Conversion.sol` on Ethereum. Design:
//! docs/specs/ipow-conversion-app.md.
//!
//! Every job asks the protocol's lowest escrow: the swap's value is kept
//! safe by this program, not by the escrow. No key can change the program
//! once its upgrade authority is removed (D59).

use anchor_lang::prelude::*;
pub use ipow_protocol::state::BlockRef;

pub mod compensation;
pub mod constants;
pub mod errors;
pub mod jobs;
pub mod money;
pub mod payment;
pub mod state;

// Instruction modules sit at the crate root and are named after their
// `Accounts` struct, to work around the anchor-lang 1.0 `#[program]` macro
// bug: it expects each `__client_accounts_<snake_case(struct)>` module at
// `crate::<that name>` (https://github.com/solana-foundation/anchor/issues/3690).
pub mod buy;
pub mod cancel;
pub mod compensate;
pub mod complete_buy;
pub mod complete_sell;
pub mod fund;
pub mod initialize;
pub mod prove_my_payment;
pub mod reclaim;
pub mod refund_sell;
pub mod sell;

pub use buy::Buy;
pub(crate) use buy::__client_accounts_buy;
pub use cancel::Cancel;
pub use compensate::Compensate;
pub(crate) use compensate::__client_accounts_compensate;
pub(crate) use cancel::__client_accounts_cancel;
pub use complete_buy::CompleteBuy;
pub(crate) use complete_buy::__client_accounts_complete_buy;
pub use complete_sell::CompleteSell;
pub(crate) use complete_sell::__client_accounts_complete_sell;
pub use fund::Fund;
pub(crate) use fund::__client_accounts_fund;
pub use initialize::Initialize;
pub(crate) use initialize::__client_accounts_initialize;
pub use prove_my_payment::ProveMyPayment;
pub(crate) use prove_my_payment::__client_accounts_prove_my_payment;
pub use reclaim::Reclaim;
pub(crate) use reclaim::__client_accounts_reclaim;
pub use refund_sell::RefundSell;
pub(crate) use refund_sell::__client_accounts_refund_sell;
pub use sell::Sell;
pub(crate) use sell::__client_accounts_sell;

// The same bug hides the accounts the generated CPI helpers use.
#[cfg(feature = "cpi")]
pub(crate) use buy::__cpi_client_accounts_buy;
#[cfg(feature = "cpi")]
pub(crate) use compensate::__cpi_client_accounts_compensate;
#[cfg(feature = "cpi")]
pub(crate) use cancel::__cpi_client_accounts_cancel;
#[cfg(feature = "cpi")]
pub(crate) use complete_buy::__cpi_client_accounts_complete_buy;
#[cfg(feature = "cpi")]
pub(crate) use complete_sell::__cpi_client_accounts_complete_sell;
#[cfg(feature = "cpi")]
pub(crate) use fund::__cpi_client_accounts_fund;
#[cfg(feature = "cpi")]
pub(crate) use initialize::__cpi_client_accounts_initialize;
#[cfg(feature = "cpi")]
pub(crate) use prove_my_payment::__cpi_client_accounts_prove_my_payment;
#[cfg(feature = "cpi")]
pub(crate) use reclaim::__cpi_client_accounts_reclaim;
#[cfg(feature = "cpi")]
pub(crate) use refund_sell::__cpi_client_accounts_refund_sell;
#[cfg(feature = "cpi")]
pub(crate) use sell::__cpi_client_accounts_sell;

declare_id!("9Argk3M83p8t5pWG92PhwEhYzysWt5n82siEWyb29w7D");

#[program]
pub mod conversion {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }

    pub fn sell(ctx: Context<Sell>, amount: u64, sats: u64, script: Vec<u8>, confirmations: u16, paid: u64) -> Result<()> {
        sell::handler(ctx, amount, sats, script, confirmations, paid)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sell_in_window(
        ctx: Context<Sell>,
        amount: u64,
        sats: u64,
        script: Vec<u8>,
        pay_from: u32,
        pay_to: u32,
        confirmations: u16,
        paid: u64,
    ) -> Result<()> {
        sell::in_window(ctx, amount, sats, script, pay_from, pay_to, confirmations, paid)
    }

    pub fn complete_sell(ctx: Context<CompleteSell>, raw_tx: Vec<u8>) -> Result<()> {
        complete_sell::handler(ctx, raw_tx)
    }

    pub fn refund_sell(ctx: Context<RefundSell>, raw_tx: Vec<u8>) -> Result<()> {
        refund_sell::handler(ctx, raw_tx)
    }

    pub fn buy(ctx: Context<Buy>, amount: u64, sats: u64, confirmations: u16, paid: u64) -> Result<()> {
        buy::handler(ctx, amount, sats, confirmations, paid)
    }

    pub fn buy_for(ctx: Context<Buy>, recipient: Pubkey, amount: u64, sats: u64, confirmations: u16, paid: u64) -> Result<()> {
        buy::for_recipient(ctx, recipient, amount, sats, confirmations, paid)
    }

    pub fn fund(ctx: Context<Fund>, script: Vec<u8>) -> Result<()> {
        fund::handler(ctx, script)
    }

    pub fn cancel(ctx: Context<Cancel>) -> Result<()> {
        cancel::handler(ctx)
    }

    pub fn complete_buy(ctx: Context<CompleteBuy>, receipt_raw: Vec<u8>, payment_raw: Vec<u8>, vout: u32) -> Result<()> {
        complete_buy::handler(ctx, receipt_raw, payment_raw, vout)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prove_my_payment(
        ctx: Context<ProveMyPayment>,
        payment_raw: Vec<u8>,
        vout: u32,
        pay_block: BlockRef,
        siblings: Vec<[u8; 32]>,
        tx_index: u64,
        top: BlockRef,
        parent_epoch_time: u32,
    ) -> Result<()> {
        prove_my_payment::handler(ctx, payment_raw, vout, pay_block, siblings, tx_index, top, parent_epoch_time)
    }

    pub fn compensate(ctx: Context<Compensate>) -> Result<()> {
        compensate::handler(ctx)
    }

    pub fn reclaim(ctx: Context<Reclaim>) -> Result<()> {
        reclaim::handler(ctx)
    }
}
