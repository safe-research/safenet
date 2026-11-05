//! V-VAL Phase 5 — settles VAL-Q6 / shared question 13 by execution.
//!
//! Does `frost-core 3.0.0`'s `round2::sign` reject a `SigningNonces` that does
//! not match the commitment in the signing package? This decides F-VAL-034's
//! *outcome*: if it rejects, a stale nonce resume landing on a restarted
//! session yields an error (a warning and no share), not a share computed from
//! a reused nonce.
//!
//! Wire in via `crates/validator/src/frost/mod.rs`:
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/V-VAL-dependency-questions/commitment_mismatch.rs"]
//! mod poc_v_val_q6;
//! ```
//! Run: cargo test -p validator --bins frost::poc_v_val_q6 -- --nocapture

use super::{keygen, preprocess, sign};
use alloy::primitives::{Address, address, keccak256};
use std::collections::BTreeMap;

const PARTICIPANTS: [Address; 3] = [
    address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
    address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"),
    address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
];
const SIGNER_A: Address = PARTICIPANTS[0];
const SIGNER_B: Address = PARTICIPANTS[2];

fn finalized_group() -> BTreeMap<Address, keygen::KeyShare> {
    let mut rng = rand::thread_rng();
    let mut secrets = BTreeMap::new();
    let mut commitments = BTreeMap::new();
    for p in PARTICIPANTS {
        let s = keygen::setup(&mut rng, p, 3, 2).unwrap();
        commitments.insert(p, s.commitment());
        secrets.insert(p, s);
    }
    let verified = PARTICIPANTS
        .into_iter()
        .map(|p| (p, keygen::verify_commitment(p, &commitments[&p]).unwrap()))
        .collect::<BTreeMap<_, _>>();
    let mut states = BTreeMap::new();
    let mut shares = BTreeMap::new();
    for (p, s) in secrets {
        let (state, share) = keygen::generate_secret_shares(s, verified.clone()).unwrap();
        states.insert(p, state);
        shares.insert(p, share);
    }
    states
        .into_iter()
        .map(|(p, state)| {
            let vs = shares
                .iter()
                .map(|(peer, share)| {
                    let (_, enc) =
                        keygen::verify_secret_share(state.group_commitments(), *peer, share)
                            .unwrap();
                    (
                        *peer,
                        keygen::verify_encrypted_secret_share(&state, *peer, enc).unwrap(),
                    )
                })
                .collect();
            (p, keygen::finalize(state, vs).unwrap())
        })
        .collect()
}

fn fresh(key_share: &keygen::KeyShare) -> preprocess::Nonces {
    let mut chunk =
        preprocess::NonceChunk::with_size(1, key_share, &mut rand::thread_rng()).unwrap();
    chunk.nonces.remove(0)
}

/// VAL-Q6: a `Nonces` value that is NOT the one whose commitment was revealed
/// must be rejected.
///
/// PASS ⇒ `frost-core` enforces the binding (`round2.rs:140-143`,
/// `Error::IncorrectCommitment`). F-VAL-034's dangerous outcome (a share over a
/// stale nonce) cannot happen; the observable is an error and no share.
#[test]
fn signing_with_a_nonce_that_does_not_match_the_revealed_commitment_is_rejected() {
    let key_shares = finalized_group();
    let message = keccak256(b"F-VAL-034: a resume from a restarted ceremony");

    let a_revealed = fresh(&key_shares[&SIGNER_A]);
    let a_stale = fresh(&key_shares[&SIGNER_A]); // a different session's nonce
    let b_nonces = fresh(&key_shares[&SIGNER_B]);

    let (a_reveal, _) = a_revealed.reveal();
    let (b_reveal, _) = b_nonces.reveal();
    let revealed = [(SIGNER_A, &a_reveal), (SIGNER_B, &b_reveal)]
        .into_iter()
        .map(|(addr, n)| (addr, sign::verify_revealed_nonces(addr, n).unwrap()))
        .collect::<BTreeMap<_, _>>();

    // Control: the matching nonce signs.
    let ok = sign::signature_share(&key_shares[&SIGNER_A], a_revealed, &revealed, &message);
    assert!(ok.is_ok(), "the matching nonce produces a share");

    // The question: the stale nonce from another session.
    let bad = sign::signature_share(&key_shares[&SIGNER_A], a_stale, &revealed, &message);
    let bad = bad.map(|_| "<a signature share>");
    println!("\n=== VAL-Q6 result ===\nstale-nonce signature_share -> {bad:?}\n");
    assert!(
        bad.is_err(),
        "VAL-Q6 REFUTED: frost-core produced a signature share from a nonce whose \
         commitment is not the one in the signing package. F-VAL-034's outcome would \
         then be nonce reuse, not a warning."
    );
}
