// PoC for F-XC-051 (Draft, no Critic yet) — and question 5 of
// rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md, which also blocks
// R4's Observation O4.
//
// NOT COMPILED, NOT RUN — no Rust toolchain on the audit machine.
//
// Append this module to the end of `crates/validator/src/frost/keygen.rs`.
// (If the F-XC-002 PoC is also installed, both modules can coexist in that
// file; they have different names.)
//
// Run:  cargo test -p validator qa_xc_051 -- --nocapture
//
// WHAT IS BEING ASKED. `verify_commitment` carries this comment
// (crates/validator/src/frost/keygen.rs:83-86):
//
//     // Note that we do not check the length of the commitments, this is
//     // enforced by the smart contract and any issues will be caught later
//     // and produce an unexpected FROST error.
//
// The second clause is an unverified claim about an upstream crate sitting in
// a security-critical decoder. Under F-VAL-060's precondition the event need
// not come from the coordinator, so the first clause's premise is also in
// question. These tests decide whether a degenerate commitment vector is
// REJECTED (the comment is right) or PANICS (the comment is wrong, and the
// panic is caught nowhere — there is no `catch_unwind` in the driver).

#[cfg(test)]
mod qa_xc_051 {
    use super::*;
    use alloy::primitives::address;

    const PARTICIPANT: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    /// A structurally valid, non-identity point: the secp256k1 generator.
    fn generator() -> bindings::Point {
        marshal::solidity_point(&k256::ProjectivePoint::GENERATOR)
    }

    /// The point the ABI decoder maps to the identity element
    /// (`crates/validator/src/frost/marshal.rs:131-135`: an all-zero
    /// `Point` decodes to `ProjectivePoint::IDENTITY`).
    fn identity() -> bindings::Point {
        bindings::Point {
            x: U256::ZERO,
            y: U256::ZERO,
        }
    }

    fn commitment(c: Vec<bindings::Point>) -> bindings::KeyGenCommitment {
        bindings::KeyGenCommitment {
            q: generator(),
            c,
            r: generator(),
            mu: U256::from(1),
        }
    }

    #[test]
    fn qa_xc_051_empty_commitment_vector_returns_an_error_and_does_not_panic() {
        // The decisive case. `frost_commitment` builds a
        // `VerifiableSecretSharingCommitment` from an EMPTY coefficient vector
        // (marshal.rs:110-119 maps over `commitment.c` with no length check),
        // and hands it to `frost_core::keys::dkg::verify_proof_of_knowledge`.
        // If that function indexes `[0]` to read the constant term, this
        // panics inside a state-machine event handler.
        let result = verify_commitment(PARTICIPANT, &commitment(vec![]));

        assert!(
            result.is_err(),
            "an empty commitment vector was ACCEPTED; the Rust performs no \
             structural validation at all and relies entirely on the contract"
        );
        println!("empty c -> {result:?}");
    }

    #[test]
    fn qa_xc_051_identity_coefficients_are_rejected() {
        // `frost_point` deliberately decodes the zero point to the identity
        // (marshal.rs:130-137) — the only decoder in this module that does
        // NOT then reject it, unlike `frost_signing_commitments`
        // (marshal.rs:167-170), which explicitly errors on an identity
        // hiding/binding point.
        let result = verify_commitment(PARTICIPANT, &commitment(vec![identity(), identity()]));

        assert!(
            result.is_err(),
            "a commitment whose coefficients are all the identity element was \
             ACCEPTED; the group verifying key derived from it is the identity"
        );
        println!("identity c -> {result:?}");
    }

    #[test]
    fn qa_xc_051_a_single_coefficient_where_the_threshold_is_two() {
        // The length case the workspace has no assertion for anywhere. A
        // commitment vector shorter than the group threshold describes a
        // lower-degree polynomial, i.e. a smaller signing threshold than the
        // group agreed on. Nothing in `verify_commitment` is given the
        // threshold to compare against — the caller has it
        // (`group.size()` at crates/validator/src/state/keygen.rs:171) and
        // does not pass it.
        let result = verify_commitment(PARTICIPANT, &commitment(vec![generator()]));

        // The expected outcome is Err (the proof of knowledge will not verify
        // against a fabricated commitment), but the point of the test is the
        // NEGATIVE one: it must not panic, and the error must be a returned
        // error rather than an abort.
        println!("single-coefficient c -> {result:?}");
        assert!(result.is_err());
    }
}
