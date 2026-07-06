//! Key-derivation primitives shared across the crate.

use alloy::primitives::B256;
use hkdf::Hkdf;
use sha2::Sha256;

/// Deterministically derives a 32-byte value from `ikm` (input keying material) using
/// HKDF-SHA256 (RFC 5869), scoped to `domain` and bound to `message`.
///
/// `domain` is used as the HKDF salt (RFC 5869 §3.1: salt lets multiple independent
/// pseudorandom keys be derived from a single `ikm`), so derivations for different use cases
/// over the same `ikm` can never collide with one another. `message` is passed as HKDF's
/// `info` parts (RFC 5869 §3.2), fed to the underlying HMAC incrementally rather than
/// concatenated upfront.
///
/// # Panics
///
/// Panics if `domain` is empty, since an empty domain provides no separation.
pub fn derive_key(ikm: &[u8], domain: &[u8], message: &[&[u8]]) -> B256 {
    assert!(!domain.is_empty(), "HKDF domain must not be empty");

    let hkdf = Hkdf::<Sha256>::new(Some(domain), ikm);
    let mut okm = [0u8; 32];
    hkdf.expand_multi_info(message, &mut okm)
        .expect("32 bytes is far below HKDF-SHA256's maximum output length");
    B256::from(okm)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independently computed (Python `hmac`/`hashlib`, mirroring RFC 5869 §2.2/§2.3), since
    /// the RFC's own published vectors use different salt/IKM/info lengths than we exercise
    /// here.
    #[test]
    fn derive_key_matches_reference_vector() {
        let ikm = b"top secret key material";
        let domain = b"safenet-sentinel-reveal-salt";
        let message = b"request-1";
        let expected: B256 = "de66ad87d39718318f7ec36177e9e2286b5c0ade3dc0de22b65e9ee55ccaab0d"
            .parse()
            .unwrap();

        assert_eq!(derive_key(ikm, domain, &[message]), expected);
    }

    #[test]
    fn derive_key_is_deterministic_and_bound_to_domain_and_message() {
        let ikm = b"top secret key material";

        assert_eq!(
            derive_key(ikm, b"domain-a", &[b"message-1"]),
            derive_key(ikm, b"domain-a", &[b"message-1"]),
        );
        assert_ne!(
            derive_key(ikm, b"domain-a", &[b"message-1"]),
            derive_key(ikm, b"domain-b", &[b"message-1"]),
        );
        assert_ne!(
            derive_key(ikm, b"domain-a", &[b"message-1"]),
            derive_key(ikm, b"domain-a", &[b"message-2"]),
        );
    }

    #[test]
    fn derive_key_treats_multi_part_message_as_its_concatenation() {
        let ikm = b"top secret key material";

        assert_eq!(
            derive_key(ikm, b"domain-a", &[b"foo", b"bar"]),
            derive_key(ikm, b"domain-a", &[b"foobar"]),
        );
    }

    #[test]
    #[should_panic(expected = "HKDF domain must not be empty")]
    fn derive_key_rejects_empty_domain() {
        derive_key(b"ikm", b"", &[b"message"]);
    }
}
