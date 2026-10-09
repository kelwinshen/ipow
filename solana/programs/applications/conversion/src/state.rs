use anchor_lang::prelude::*;

/// The program's one settings record. Its address is also the key this
/// application registered with the protocol: it signs the application's
/// calls, and receives the escrow share of a slashed job.
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub swap_count: u64,
    /// Escrow shares of slashed jobs, withdrawn and not yet passed on.
    pub compensation: u64,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum Side {
    Sell,
    Buy,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace, Debug)]
pub enum SwapState {
    /// Sell: waiting for the operator's payment. Buy: waiting for the
    /// operator to lock the coin.
    Open,
    /// Buy only: the operator locked the coin.
    Funded,
    /// The coin went to the operator (sell) or to the user (buy).
    Done,
    /// Sell: the coin went back to the user.
    Refunded,
    /// Buy: the operator did not lock the coin in time.
    Cancelled,
    /// Buy: the coin went back to the operator.
    Reclaimed,
}

/// A swap. Holds the coin itself when the coin is SOL; a token is held in
/// the swap's associated token account.
#[account]
#[derive(InitSpace)]
pub struct Swap {
    pub id: u64,
    pub side: Side,
    pub state: SwapState,
    pub user: Pubkey,
    /// `Pubkey::default()` for SOL.
    pub mint: Pubkey,
    pub amount: u64,
    /// Sell: the least the user accepts. Buy: what the user pays.
    pub sats: u64,
    pub job_id: u64,
    /// Sell: the user's script, to be paid. Buy: the operator's script, set
    /// when it locks the coin.
    #[max_len(100)]
    pub script: Vec<u8>,
    /// Whether the user received this application's share of the job's
    /// escrow after a slash.
    pub compensated: bool,
    pub bump: u8,
    /// Sell in a tunnel only (T1): the Bitcoin blocks its payment must be
    /// mined in, those of the buy it pays on another network; zero for none.
    /// A payment proven outside them refunds the user.
    pub pay_from: u32,
    pub pay_to: u32,
}

impl Swap {
    /// Whether a tunnel's sell was proven by a transaction mined outside its
    /// payment window. A sell with no window never is.
    pub fn outside_window(&self, proof_height: u32) -> bool {
        self.pay_from != 0 && (proof_height < self.pay_from || proof_height > self.pay_to)
    }
}

/// Marks an operator's script used, so a payment to it belongs to one swap.
#[account]
#[derive(InitSpace)]
pub struct ScriptRecord {
    pub swap_id: u64,
    pub bump: u8,
}
