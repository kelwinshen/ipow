//! The iPoW protocol on EVM networks, for the node: the contracts of
//! `programmable-network/ethereum/contracts/protocol`.

pub mod contracts;
pub mod network;

#[cfg(feature = "testing")]
pub mod testing;
pub mod conversion;
pub mod vault;
