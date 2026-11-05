// rv-xc run 3: the run-1 PoC for F-XC-051 (poc/F-XC-051/append-to-crates-validator-src-frost-keygen.rs,
// written by QA-XC and never run), adapted to 361aaf3 and run for the first time.
//
// Appended to the end of `crates/validator/src/frost/keygen.rs`, then reverted with
// `git checkout -- crates/validator/src/frost/keygen.rs`.
// Run: cargo test --offline --locked -p validator --bins frost::keygen::qa_xc_051 -- --nocapture
//
// Why the fixtures changed: since #1027 `KeyGenCommitment` is `{ q, pop, c, pok }` and
// `verify_commitment` checks the proof of possession of `q` *before* the proof of knowledge
// of `c[0]`. The run-1 fixtures (`q = r = G`, `mu = 1`) would now fail at the possession check
// and never reach the code under test. So every case starts from a commitment built by the
// crate's own `setup` + `Secrets::commitment` (valid `q`, `pop`, `c`, `pok`) and changes only
// `c`, which the proof of possession does not cover.

#[cfg(test)]
mod qa_xc_051 {
    use super::*;
    use alloy::primitives::address;

    const PARTICIPANT: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    /// The point the ABI decoder maps to the identity element
    /// (`marshal::frost_point`: an all-zero `Point` decodes to `ProjectivePoint::IDENTITY`).
    fn identity() -> bindings::Point {
        bindings::Point {
            x: U256::ZERO,
            y: U256::ZERO,
        }
    }

    /// A valid onchain commitment of `me` for a 3-of-2 group (threshold 2, so `c.len() == 2`).
    fn valid(me: Address) -> bindings::KeyGenCommitment {
        setup(&mut rand::thread_rng(), me, 3, 2).unwrap().commitment()
    }

    #[test]
    fn qa_xc_051_control_the_unmodified_commitment_verifies() {
        let commitment = valid(PARTICIPANT);
        assert_eq!(commitment.c.len(), 2);
        let result = verify_commitment(PARTICIPANT, &commitment);
        println!("control (c.len() = 2) -> ok = {}", result.is_ok());
        assert!(result.is_ok());
    }

    #[test]
    fn qa_xc_051_empty_commitment_vector_returns_an_error_and_does_not_panic() {
        let mut commitment = valid(PARTICIPANT);
        commitment.c = vec![];
        let result = verify_commitment(PARTICIPANT, &commitment);
        println!("empty c -> {result:?}");
        assert!(result.is_err(), "an empty commitment vector was accepted");
    }

    #[test]
    fn qa_xc_051_identity_constant_term_is_rejected() {
        let mut commitment = valid(PARTICIPANT);
        commitment.c[0] = identity();
        let result = verify_commitment(PARTICIPANT, &commitment);
        println!("identity c[0] -> {result:?}");
        assert!(result.is_err());
    }

    #[test]
    fn qa_xc_051_identity_coefficients_are_rejected() {
        // The run-1 case: every coefficient is the identity.
        let mut commitment = valid(PARTICIPANT);
        commitment.c = vec![identity(), identity()];
        let result = verify_commitment(PARTICIPANT, &commitment);
        println!("identity c (all) -> {result:?}");
        assert!(result.is_err());
    }

    #[test]
    fn qa_xc_051_identity_higher_coefficient_is_accepted() {
        // c[1] = identity with a valid proof of knowledge for c[0]: accepted, as #1025 intends
        // (identity points in round-1 packages are persisted, culprits found through complaints).
        let mut commitment = valid(PARTICIPANT);
        commitment.c[1] = identity();
        let result = verify_commitment(PARTICIPANT, &commitment);
        println!("identity c[1] (valid pok for c[0]) -> accepted = {}", result.is_ok());
        assert!(result.is_ok());
    }

    #[test]
    fn qa_xc_051_a_single_coefficient_where_the_threshold_is_two() {
        // The residual run 2 left untraced. The proof of knowledge covers `c[0]` only, so a
        // commitment truncated to its constant term still verifies: `verify_commitment` is not
        // given the threshold and does not check the length (keygen.rs:89-91). On the honest
        // path the coordinator rejects such a commitment (`c.length == threshold`), so this is
        // reachable only through the F-VAL-060 / F2-VAL-061 injection precondition.
        let mut commitment = valid(PARTICIPANT);
        commitment.c.truncate(1);
        let result = verify_commitment(PARTICIPANT, &commitment);
        println!("single-coefficient c (threshold 2) -> accepted = {}", result.is_ok());
        assert!(result.is_ok(), "the length is now checked: the residual is closed");

        // What the group computation then does with it (frost-core `sum_commitments` sizes the
        // sum by the FIRST commitment in address order and errors on a shorter later one).
        let low = address!("0000000000000000000000000000000000000001");
        let high = address!("ffffffffffffffffffffffffffffffffffffffff");
        let full_low = verify_commitment(low, &valid(low)).unwrap();
        let full_high = verify_commitment(high, &valid(high)).unwrap();
        let mut short_low = valid(low);
        short_low.c.truncate(1);
        let short_low = verify_commitment(low, &short_low).unwrap();
        let mut short_high = valid(high);
        short_high.c.truncate(1);
        let short_high = verify_commitment(high, &short_high).unwrap();

        let short_last = group_commitments(BTreeMap::from([(low, full_low), (high, short_high)]));
        println!("group_commitments with the short commitment sorted last -> {short_last:?}");
        let short_first = group_commitments(BTreeMap::from([(low, short_low), (high, full_high)]));
        match &short_first {
            Ok(group) => println!(
                "group_commitments with the short commitment sorted first -> Ok, group commitment length {}",
                group.group_commitment.coefficients().len()
            ),
            Err(err) => println!("group_commitments with the short commitment sorted first -> Err({err:?})"),
        }
    }
}
