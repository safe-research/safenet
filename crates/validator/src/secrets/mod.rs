//! Reorg-resistant storage and generation of locally-produced secrets.

pub mod nonces;
pub mod store;

pub use store::SecretStore;
