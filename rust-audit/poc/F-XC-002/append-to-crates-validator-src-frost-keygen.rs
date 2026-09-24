// PoC for F-XC-002 (and F-VAL-062, F-CORE-036, and question 1 of
// rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md).
//
// NOT COMPILED, NOT RUN — there is no Rust toolchain on the audit machine.
//
// Append this module verbatim to the end of
// `crates/validator/src/frost/keygen.rs`. It must live inside that file:
// `KeyShare::dummy()` is `#[cfg(test)] pub(crate)`, `Secrets`' fields are
// private, and `validator` is a binary-only crate with no `tests/` directory,
// so an integration test cannot reach any of this.
//
// Run:  cargo test -p validator qa_xc_002 -- --nocapture
//
// PASS  (both tests green) => `frost-core` 3.0.0 redacts the signing share and
//       the round-1 secret polynomial in `Debug`. F-XC-002 / F-VAL-062 /
//       F-CORE-036 drop to Informational hygiene, and this test is worth
//       keeping so a dependency bump cannot silently change the answer.
// FAIL  (either test red) => a FROST key share is written verbatim into the
//       validator's log by `tracing::warn!(?effect, ...)`
//       (`crates/validator/src/service/effect.rs:249`) at the shipped
//       `log_filter = "info"`, on any secret-store write failure. That is
//       PROMPT.md §8's first Critical bullet ("leaks or allows recovery of
//       FROST key shares") and F-XC-002 / F-VAL-062 must be re-scored from
//       Medium to Critical.
//
// The failure message prints the offending rendering, so a red run is
// self-documenting. `--nocapture` also prints both renderings on a green run,
// which is the artefact to save under this directory as the `E1` evidence.

#[cfg(test)]
mod qa_xc_002 {
    use super::*;
    use alloy::primitives::address;

    /// An address from the repository's own fixtures (Anvil account #0, as
    /// used by `crates/validator/src/frost/mod.rs:29`).
    const PARTICIPANT: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    /// A recognisable, non-secret-looking marker value used as a stand-in
    /// signing share. Chosen so that *any* faithful rendering of the scalar —
    /// hex, decimal, or a byte array — contains a substring this test can look
    /// for, which a generic "does it contain a 64-character hex run?" check
    /// cannot do without false-alarming on the public verifying key.
    const MARKER: u64 = 0xdead_beef;

    /// Builds a `KeyShare` whose signing share is `MARKER`, mirroring
    /// `KeyShare::dummy()` exactly except for that scalar.
    fn marked_key_share() -> KeyShare {
        let secret = marshal::frost_scalar(&U256::from(MARKER)).unwrap();
        KeyShare(frost_secp256k1::keys::KeyPackage::new(
            frost_secp256k1::Identifier::try_from(1).unwrap(),
            frost_secp256k1::keys::SigningShare::new(secret),
            frost_secp256k1::keys::VerifyingShare::new(k256::ProjectivePoint::GENERATOR),
            frost_secp256k1::VerifyingKey::new(k256::ProjectivePoint::GENERATOR),
            1,
        ))
    }

    #[test]
    fn key_share_debug_does_not_print_the_signing_share() {
        let rendered = format!("{:?}", marked_key_share());
        println!("KeyShare Debug: {rendered}");

        let lowered = rendered.to_ascii_lowercase();
        assert!(
            !lowered.contains("deadbeef"),
            "KeyShare's derived Debug printed the signing share as hex: {rendered}"
        );
        assert!(
            !rendered.contains(&MARKER.to_string()),
            "KeyShare's derived Debug printed the signing share as decimal: {rendered}"
        );
        // The byte-array rendering of the same scalar, big-endian, is the
        // third form frost-core could plausibly emit.
        assert!(
            !rendered.contains("222, 173, 190, 239"),
            "KeyShare's derived Debug printed the signing share as bytes: {rendered}"
        );

        // Sanity check on the test itself: the marker really is in this value,
        // so a green run above means redaction and not a mis-built fixture.
        assert!(
            format!("{:?}", U256::from(MARKER)).to_ascii_lowercase().contains("deadbeef")
                || U256::from(MARKER).to_string().contains(&MARKER.to_string()),
            "the marker scalar is not recognisable in any rendering; fix the fixture"
        );
    }

    #[test]
    fn secrets_debug_does_not_print_the_round1_secret_polynomial() {
        let mut rng = rand::thread_rng();
        let secrets = setup(&mut rng, PARTICIPANT, 3, 2).unwrap();

        // The value that must not appear: the round-1 secret share scalar.
        // `finalize` compares against exactly this accessor
        // (`crates/validator/src/frost/keygen.rs`, the
        // `round2_me_package.signing_share().to_scalar() != ...secret_share()`
        // check), so the call is known to exist in this checkout.
        //
        // MECHANICAL FIX IF IT DOES NOT COMPILE: if `secret_share()` returns a
        // reference rather than a value, write `*secrets.secret_package.secret_share()`.
        let secret: k256::Scalar = secrets.secret_package.secret_share();
        let secret_hex = format!("{:x}", marshal::solidity_scalar(&secret));
        let secret_hex = format!("{secret_hex:0>64}");

        let rendered = format!("{:?}", secrets);
        println!("Secrets Debug: {rendered}");

        assert!(
            !rendered.to_ascii_lowercase().contains(&secret_hex),
            "Secrets' derived Debug printed the round-1 secret share ({secret_hex}): {rendered}"
        );
        // `EncryptionKey` inside `Secrets` has a hand-written redacting Debug
        // (`crates/validator/src/frost/ecdh.rs:50-53`), so this substring must
        // be present. If it is missing, the redaction convention regressed and
        // the whole finding is worse than filed.
        assert!(
            rendered.contains("redacted"),
            "EncryptionKey's redacting Debug did not run inside Secrets: {rendered}"
        );
    }
}
