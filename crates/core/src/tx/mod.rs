//! Reliable onchain transaction submission.
//!
//! Safenet services submit transactions to advance the protocol onchain. This
//! module provides a transaction queue that accepts transactions to execute and
//! reliably gets them onchain: managing nonces, signing and submitting via a
//! local [`account`], and resubmitting with bumped fees when a transaction is
//! stuck.

#![allow(dead_code)]

mod fees;
pub mod signer;

pub use self::signer::Signer;
