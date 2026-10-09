//! The iPoW protocol on EVM networks, for the node: the contracts of
//! `evm/contracts/protocol`.

pub mod contracts;
pub mod network;

#[cfg(feature = "testing")]
pub mod testing;
pub mod conversion;
pub mod vault;
