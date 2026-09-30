//! The iPoW node: runs each chosen role on each chosen network as a task of
//! its own. A task that fails or panics is started again after a pause; the
//! other tasks go on.

pub mod attester;
pub mod bitcoin;
pub mod defence;
pub mod guardian;
pub mod light;
pub mod operator;
pub use ipow_protocol_core::secrets;
pub mod wallet;
pub mod supervisor;
pub mod swaps;
pub mod vault;
