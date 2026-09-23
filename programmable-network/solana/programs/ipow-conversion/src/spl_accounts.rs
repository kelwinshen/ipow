use anchor_lang::prelude::*;

/// Every value-moving instruction that touches `Conversion.native_amount`
/// (deposit/payout — never the auction stake, commit fee, or bounty, which
/// stay native SOL regardless of `token_mint`, same convention `ipow-
/// message-relay` already uses) needs the exact same accounts, present
/// whether or not this particular conversion is SPL: `mint`, `token_
/// program`, `associated_token_program` are only ever read/invoked when
/// `token_mint != Pubkey::default()`, so a native conversion can pass
/// harmless placeholders (its own program id works fine) for all three
/// without them ever being touched. A separate top-level module (not
/// nested in `utils.rs`) because anchor-lang 1.0's `#[program]` macro
/// requires a composite `Accounts` struct's auto-generated
/// `__client_accounts_*`/`__cpi_client_accounts_*` support modules
/// reachable at a bare `crate::<name>` path — see the note in `ipow`'s own
/// `lib.rs` for the same constraint applied to instruction modules.
#[derive(Accounts)]
pub struct SplAccounts<'info> {
    /// CHECK: only read (as `Mint::LEN`-shaped data) when `token_mint !=
    /// default`, via `associated_token::create_idempotent`'s own
    /// deserialization — never written here.
    pub mint: UncheckedAccount<'info>,
    /// CHECK: SPL Token program — checked against `anchor_spl::token::ID`
    /// before any CPI into it.
    pub token_program: UncheckedAccount<'info>,
    /// CHECK: Associated Token program — checked against `anchor_spl::
    /// associated_token::ID` before any CPI into it.
    pub associated_token_program: UncheckedAccount<'info>,
}
