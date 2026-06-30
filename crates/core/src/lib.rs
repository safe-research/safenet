//! # Safenet Core
//!
//! The Safenet core crate contains shared components used across various
//! Safenet services. It provides shared logic for:
//!
//! - Logging, metrics and observability.
//! - Indexing onchain blocks and events, guaranteeing that they appear in order
//!   and correctly handle chain reorgs.
//! - Utilities for managing service state that needs to be preserved across
//!   restarts and roll back in case of reorgs.
//! - Reliable transaction submission with all its complexities.

pub mod driver;
pub mod index;
pub mod observability;
pub mod serialization;
pub mod state;
pub mod tx;

pub use self::driver::{Driver, Service};
