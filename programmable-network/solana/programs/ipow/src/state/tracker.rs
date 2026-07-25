use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct HeightTracker {
    pub count: u64,
}
