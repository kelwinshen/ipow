//! The iPoW protocol on Solana, for the node: the programs of
//! `programmable-network/solana/programs/{ipow-light-client,ipow-protocol}`.

pub mod programs {
    use anchor_lang;
    anchor_lang::declare_program!(ipow_protocol);
    anchor_lang::declare_program!(ipow_light_client);
    anchor_lang::declare_program!(conversion);
    anchor_lang::declare_program!(ipow_vault);
}

pub mod chain;
pub mod conversion;
pub mod network;
pub mod rules;
pub mod vault;

#[cfg(feature = "testing")]
pub mod testing;
