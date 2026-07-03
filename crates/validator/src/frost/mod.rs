//! A thin Safenet-specific layer over the ZCash Foundation FROST crates.
//!
//! Safenet uses the standard RFC 9591 FROST(secp256k1, SHA-256) ciphersuite.
//!
//! This module provides an interface that is compatible with the onchain
//! FROST coordinator contract, internally managing the marshalling between
//! [`frost-secp256k1`] values and their Solidity ABI representations.

#![expect(dead_code)]

pub mod ecdh;
pub mod error;
pub mod marshal;
