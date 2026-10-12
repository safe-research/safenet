//! CA3-FIX (2026-10-09): F-VAL-001 variants the run-3 re-validation did not
//! list, at the Aegis group size (5 members, threshold 3). Child module of
//! `crate::frost`, wired the same way as `../run3/poc_variants.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "<abs path>/rust-audit/poc/F-VAL-001/ca3-fix/ca3fix_frost.rs"]
//! mod ca3fix_f_val_001;
//! ```
//!
//! The PoP reuses the DKG PoK scheme with no separate domain. These tests try
//! to make that reuse (or a whole copied commitment) pass under the impostor's
//! identifier.

use super::{error::Error, keygen};
use crate::bindings;
use alloy::primitives::{Address, address};
use std::collections::BTreeMap;

const MEMBERS: [Address; 5] = [
    address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
    address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"), // VICTIM (A)
    address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
    address!("90F79bf6EB2c4f870365E785982E1f101E93b906"),
    address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65"), // IMPOSTOR (M)
];
const VICTIM: Address = MEMBERS[1];
const IMPOSTOR: Address = MEMBERS[4];
const COUNT: u16 = 5;
const THRESHOLD: u16 = 3;

fn setup_all() -> BTreeMap<Address, bindings::KeyGenCommitment> {
    let mut rng = rand::thread_rng();
    MEMBERS
        .into_iter()
        .map(|p| {
            let s = keygen::setup(&mut rng, p, COUNT, THRESHOLD).unwrap();
            (p, s.commitment())
        })
        .collect()
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

fn copy_point(p: &bindings::Point) -> bindings::Point {
    bindings::Point { x: p.x, y: p.y }
}

/// `verify_commitment` must reject the impostor's commitment with the
/// impostor as culprit while every honest commitment still verifies.
fn assert_impostor_rejected(label: &str, c: &BTreeMap<Address, bindings::KeyGenCommitment>) {
    for p in MEMBERS {
        let result = keygen::verify_commitment(p, &c[&p]);
        if p == IMPOSTOR {
            match result {
                Err(Error::Participant { culprit, cause }) => {
                    assert_eq!(culprit, IMPOSTOR, "{label}: culprit must be the impostor");
                    println!("{label}: impostor rejected, cause = {cause:?}");
                }
                Err(other) => panic!("{label}: rejected with a non-participant error {other:?}"),
                Ok(_) => panic!("{label}: impostor commitment ACCEPTED"),
            }
        } else {
            result.unwrap_or_else(|e| panic!("{label}: honest {p} rejected: {e:?}"));
        }
    }
}

/// W0: the original route (q := q_A, own PoP) at the Aegis size.
#[test]
fn w0_original_route_at_aegis_size_rejected() {
    let mut c = setup_all();
    let q_a = copy_point(&c[&VICTIM].q);
    c.get_mut(&IMPOSTOR).unwrap().q = q_a;
    assert_impostor_rejected("w0 q := q_A at n=5 t=3", &c);
}

/// W1: PoK reused as PoP across participants. M registers q := c_A[0] (the
/// victim's public polynomial constant) with the victim's PoK of c_A[0] as its
/// PoP, and separately with its own PoP.
#[test]
fn w1_victim_pok_as_pop_rejected() {
    let c0 = setup_all();
    let c_a0 = copy_point(&c0[&VICTIM].c[0]);

    let mut c = c0.clone();
    {
        let m = c.get_mut(&IMPOSTOR).unwrap();
        m.q = copy_point(&c_a0);
        m.pop = copy_sig(&c0[&VICTIM].pok);
    }
    assert_impostor_rejected("w1a q := c_A[0], pop := pok_A", &c);

    let mut c = c0.clone();
    c.get_mut(&IMPOSTOR).unwrap().q = c_a0;
    assert_impostor_rejected("w1b q := c_A[0], own pop", &c);
}

/// W2: the whole victim commitment (q, pop, c, pok), every proof valid for the
/// victim, replayed under the impostor's address (a mempool front-run or a
/// replay from an earlier ceremony).
#[test]
fn w2_full_commitment_copy_rejected() {
    let mut c = setup_all();
    let a = c[&VICTIM].clone();
    c.insert(IMPOSTOR, a);
    assert_impostor_rejected("w2 full copy of A's commitment", &c);
}

/// W3: own q and PoP, but the victim's polynomial commitment and PoK.
#[test]
fn w3_victim_polynomial_copy_rejected() {
    let mut c = setup_all();
    let c_a = c[&VICTIM].c.clone();
    let pok_a = copy_sig(&c[&VICTIM].pok);
    {
        let m = c.get_mut(&IMPOSTOR).unwrap();
        m.c = c_a;
        m.pok = pok_a;
    }
    assert_impostor_rejected("w3 c, pok := A's", &c);
}
