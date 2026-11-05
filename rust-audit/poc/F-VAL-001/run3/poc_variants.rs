//! RUN 3 (rv-val, 2026-10-08): F-VAL-001 variants against `361aaf3`, after
//! #1027 (`14750ea`) added a proof of possession (PoP) for the encryption key
//! `q`. Wired in as a child of `crate::frost`, the same way as `../poc.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "<abs path>/rust-audit/poc/F-VAL-001/run3/poc_variants.rs"]
//! mod poc_f_val_001_run3;
//! ```
//!
//! Each `v*` test drives the real `keygen::setup` / `verify_commitment` path
//! with the impostor `M` registering a key that would let it share the victim
//! `A`'s ECDH pad, and asserts the commitment is rejected with `M` as the
//! culprit. `v8` is the control: a key `M` really owns verifies, and the pads
//! it can harvest by complaining then open none of the victim's slots.

use super::{error::Error, keygen, marshal, participants};
use crate::bindings;
use alloy::primitives::{Address, U256, address};
use k256::{ProjectivePoint, Scalar};
use std::collections::BTreeMap;

const PARTICIPANTS: [Address; 7] = [
    address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
    address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"),
    address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"), // VICTIM (A)
    address!("90F79bf6EB2c4f870365E785982E1f101E93b906"),
    address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65"),
    address!("9965507D1a55bcC2695C58ba16FB37d819B0A4dc"),
    address!("976EA74026E726554dB657fA54763abd0C3a0aa9"), // IMPOSTOR (M)
];
const VICTIM: Address = PARTICIPANTS[2];
const IMPOSTOR: Address = PARTICIPANTS[6];
const COUNT: u16 = 7;
const THRESHOLD: u16 = 4;

fn setup_all() -> (
    BTreeMap<Address, keygen::Secrets>,
    BTreeMap<Address, bindings::KeyGenCommitment>,
) {
    let mut rng = rand::thread_rng();
    let mut secrets = BTreeMap::new();
    let mut commitments = BTreeMap::new();
    for p in PARTICIPANTS {
        let s = keygen::setup(&mut rng, p, COUNT, THRESHOLD).unwrap();
        commitments.insert(p, s.commitment());
        secrets.insert(p, s);
    }
    (secrets, commitments)
}

fn point(p: &bindings::Point) -> ProjectivePoint {
    marshal::frost_point(p).unwrap()
}

fn sol(p: &ProjectivePoint) -> bindings::Point {
    marshal::solidity_point(p)
}

fn copy_sig(s: &bindings::Signature) -> bindings::Signature {
    bindings::Signature {
        r: bindings::Point {
            x: s.r.x,
            y: s.r.y,
        },
        z: s.z,
    }
}

/// Asserts that `verify_commitment` rejects `M`'s tampered commitment and names
/// `M` as the culprit, and that every honest commitment still verifies.
fn assert_impostor_rejected(
    label: &str,
    commitments: &BTreeMap<Address, bindings::KeyGenCommitment>,
) {
    for p in PARTICIPANTS {
        let result = keygen::verify_commitment(p, &commitments[&p]);
        if p == IMPOSTOR {
            match result {
                Err(Error::Participant { culprit, cause }) => {
                    assert_eq!(culprit, IMPOSTOR, "{label}: culprit must be the impostor");
                    println!("{label}: impostor rejected, cause = {cause:?}");
                }
                Err(other) => panic!("{label}: rejected with a non-participant error {other:?}"),
                Ok(_) => panic!("{label}: impostor commitment ACCEPTED, F-VAL-001 step 2 holds"),
            }
        } else {
            result.unwrap_or_else(|e| panic!("{label}: honest {p} rejected: {e:?}"));
        }
    }
}

/// V1, the original route: `q := q_A` verbatim, `M` keeps its own PoP.
#[test]
fn v1_copy_q_keep_own_pop_rejected() {
    let (_, mut c) = setup_all();
    let q_a = c[&VICTIM].q.clone();
    c.get_mut(&IMPOSTOR).unwrap().q = q_a;
    assert_impostor_rejected("v1 copy q, own pop", &c);
}

/// V2: `M` copies the victim's `q` and the victim's PoP, both verbatim from the
/// victim's `KeyGenCommitted` event. The PoP is bound to the victim's
/// identifier, so it fails under `M`'s.
#[test]
fn v2_copy_q_and_pop_verbatim_rejected() {
    let (_, mut c) = setup_all();
    let q_a = c[&VICTIM].q.clone();
    let pop_a = copy_sig(&c[&VICTIM].pop);
    let m = c.get_mut(&IMPOSTOR).unwrap();
    m.q = q_a;
    m.pop = pop_a;
    assert_impostor_rejected("v2 copy q and pop", &c);
}

/// V3: the scalar-related key the finding warned a uniqueness check would miss,
/// `q_M = 2 * q_A`, with `M`'s own PoP and with the victim's PoP.
#[test]
fn v3_scalar_related_q_rejected() {
    let (_, c0) = setup_all();
    let q_a = point(&c0[&VICTIM].q);
    let related = sol(&(q_a * Scalar::from(2u64)));
    let mut c = c0.clone();
    c.get_mut(&IMPOSTOR).unwrap().q = related.clone();
    assert_impostor_rejected("v3a 2*q_A, own pop", &c);
    let mut c = c0.clone();
    let pop_a = copy_sig(&c0[&VICTIM].pop);
    let m = c.get_mut(&IMPOSTOR).unwrap();
    m.q = related;
    m.pop = pop_a;
    assert_impostor_rejected("v3b 2*q_A, victim pop", &c);
}

/// V4: the additive related key from F2-VAL-001, `q_M = q_A + G`.
#[test]
fn v4_additive_related_q_rejected() {
    let (_, mut c) = setup_all();
    let q_a = point(&c[&VICTIM].q);
    c.get_mut(&IMPOSTOR).unwrap().q = sol(&(q_a + ProjectivePoint::GENERATOR));
    assert_impostor_rejected("v4 q_A + G, own pop", &c);
}

/// V5: the negated key `q_M = -q_A`. The pad keeps only the x-coordinate, so
/// `pad(B, -q_A) == pad(B, q_A)`; this is the related key that needs no scalar
/// at all. Tried with `M`'s own PoP and with the victim's PoP negated
/// (`R -> -R`, `z -> -z`), which satisfies the Schnorr equation for `-q_A`
/// under the victim's challenge but not under the challenge for `M`.
#[test]
fn v5_negated_q_rejected() {
    let (_, c0) = setup_all();
    let q_a = point(&c0[&VICTIM].q);
    let neg = sol(&(-q_a));
    assert_eq!(neg.x, c0[&VICTIM].q.x, "negation keeps the x-coordinate");

    let mut c = c0.clone();
    c.get_mut(&IMPOSTOR).unwrap().q = neg.clone();
    assert_impostor_rejected("v5a -q_A, own pop", &c);

    let pop_a = &c0[&VICTIM].pop;
    let r_neg = sol(&(-point(&pop_a.r)));
    let z_neg = marshal::solidity_scalar(&(-marshal::frost_scalar(&pop_a.z).unwrap()));
    let mut c = c0.clone();
    let m = c.get_mut(&IMPOSTOR).unwrap();
    m.q = neg;
    m.pop = bindings::Signature { r: r_neg, z: z_neg };
    assert_impostor_rejected("v5b -q_A, negated victim pop", &c);
}

/// V6: a PoP forged without the discrete log, random `R` and `z`.
#[test]
fn v6_random_pop_rejected() {
    let (_, mut c) = setup_all();
    let q_a = c[&VICTIM].q.clone();
    let k = Scalar::from(0x1234_5678_9abc_def0u64);
    let m = c.get_mut(&IMPOSTOR).unwrap();
    m.q = q_a;
    m.pop = bindings::Signature {
        r: sol(&(ProjectivePoint::GENERATOR * k)),
        z: U256::from(0xdead_beefu64),
    };
    assert_impostor_rejected("v6 copy q, random pop", &c);
}

/// V7: `q` at the identity with the trivial "proof" `R = z*G`, which satisfies
/// `z*G == R + c*q` for any challenge when `q` is the identity.
#[test]
fn v7_identity_q_trivial_pop_rejected() {
    let (_, mut c) = setup_all();
    let z = Scalar::from(7u64);
    let m = c.get_mut(&IMPOSTOR).unwrap();
    m.q = bindings::Point {
        x: U256::ZERO,
        y: U256::ZERO,
    };
    m.pop = bindings::Signature {
        r: sol(&(ProjectivePoint::GENERATOR * z)),
        z: marshal::solidity_scalar(&z),
    };
    assert_impostor_rejected("v7 identity q, trivial pop", &c);
}

fn xor(a: U256, b: U256) -> U256 {
    a ^ b
}

fn slot(sender: Address, recipient: Address) -> usize {
    let mut sorted = PARTICIPANTS.to_vec();
    sorted.sort();
    sorted
        .iter()
        .filter(|a| **a != sender)
        .position(|a| *a == recipient)
        .unwrap()
}

/// V8, control: `M` registers a key it owns (here `q_M = c_M[0]`, reusing its
/// PoK as the PoP, which is valid because the two proofs share one scheme).
/// The commitment verifies, the ceremony runs, `M` harvests every pad by
/// complaining against each peer, and the harvested pads open none of the
/// victim's slots: the precondition of F-VAL-001 (`M` shares `A`'s pad) is
/// unreachable with a key `M` can prove.
#[test]
fn v8_owned_key_harvest_opens_no_victim_slot() {
    let (secrets, mut c) = setup_all();
    {
        let m = c.get_mut(&IMPOSTOR).unwrap();
        m.q = bindings::Point {
            x: m.c[0].x,
            y: m.c[0].y,
        };
        m.pop = copy_sig(&m.pok);
    }
    let verified = PARTICIPANTS
        .into_iter()
        .map(|p| (p, keygen::verify_commitment(p, &c[&p]).unwrap()))
        .collect::<BTreeMap<_, _>>();
    println!("v8: q_M = c_M[0] with pok reused as pop is ACCEPTED (M owns it)");

    let mut states = BTreeMap::new();
    let mut published = BTreeMap::new();
    for (p, s) in secrets {
        let (state, share) = keygen::generate_secret_shares(s, verified.clone()).unwrap();
        states.insert(p, state);
        published.insert(p, share);
    }

    let mut opened = 0;
    for b in PARTICIPANTS {
        if b == IMPOSTOR || b == VICTIM {
            continue;
        }
        let revealed = keygen::reveal_secret_share(&states[&b], IMPOSTOR).unwrap();
        let pad = xor(revealed, published[&b].f[slot(b, IMPOSTOR)]);
        let to_victim = xor(published[&b].f[slot(b, VICTIM)], pad);
        let truth = keygen::reveal_secret_share(&states[&b], VICTIM).unwrap();
        if to_victim == truth {
            opened += 1;
        }
        let from_victim = xor(published[&VICTIM].f[slot(VICTIM, b)], pad);
        let truth = keygen::reveal_secret_share(&states[&VICTIM], b).unwrap();
        if from_victim == truth {
            opened += 1;
        }
    }
    assert_eq!(opened, 0, "a harvested pad opened a victim slot");
    println!("v8: 5 harvested pads opened 0 of 10 victim slots");
    let _ = participants::identifier(VICTIM);
}
