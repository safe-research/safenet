//! Address-derived FROST participant identifiers.
//!
//! Safenet keys participants by their Ethereum address rather than FROST's
//! default sequential identifiers. The identifier is derived with the
//! ciphersuite's `HID` hash (`hash_to_field` over the domain
//! `"FROST-secp256k1-SHA256-v1id"`), which is exactly what the TypeScript
//! `deriveParticipantId` (`hid(address)`) and the onchain
//! `FROST.identifier(address)` compute, so the three agree by construction.

use alloy::primitives::Address;
use frost_secp256k1::Identifier;

/// Derives the FROST [`Identifier`] for an Ethereum address.
///
/// Port of `deriveParticipantId` in `validator/src/frost/identifier.ts`.
pub fn identifier(address: Address) -> Identifier {
    Identifier::derive(address.as_slice())
        .expect("FROST(secp256k1, SHA-256) always supports identifier derivation")
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    #[test]
    fn identifier_is_deterministic_and_distinct() {
        let a = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
        let b = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        assert_eq!(identifier(a), identifier(a));
        assert_ne!(identifier(a), identifier(b));
    }
}
