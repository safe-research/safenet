//! RUN 3 (rv-val, 2026-10-08): F-VAL-002 at `361aaf3`, after #1027 added a
//! proof of possession for `q`. Child module of `crate::frost`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "<abs path>/rust-audit/poc/F-VAL-002/run3/poc_run3.rs"]
//! mod poc_f_val_002_run3;
//! ```
//!
//! An honest 3-of-5 ceremony through the real `keygen` API, every key owned
//! and proven. The test asserts, from public data only, (1) the two-time pad:
//! for every pair, `c_{X->Y} XOR c_{Y->X} == f_X(Y) XOR f_Y(X)`; and (2) that
//! one complaint response `f_X(P)` lets any observer decrypt the plaintiff's
//! own share `f_P(X)` from `c_{P->X}`.

use super::keygen;
use crate::bindings;
use alloy::primitives::{Address, U256, address};
use std::collections::BTreeMap;

const PARTICIPANTS: [Address; 5] = [
    address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"),
    address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"),
    address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"),
    address!("90F79bf6EB2c4f870365E785982E1f101E93b906"),
    address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65"),
];
const COUNT: u16 = 5;
const THRESHOLD: u16 = 3;

fn slot(sender: Address, recipient: Address) -> usize {
    let mut sorted = PARTICIPANTS.to_vec();
    sorted.sort();
    sorted
        .iter()
        .filter(|a| **a != sender)
        .position(|a| *a == recipient)
        .unwrap()
}

fn ceremony() -> (
    BTreeMap<Address, keygen::SharingState>,
    BTreeMap<Address, bindings::KeyGenSecretShare>,
) {
    let mut rng = rand::thread_rng();
    let mut secrets = BTreeMap::new();
    let mut commitments = BTreeMap::new();
    for p in PARTICIPANTS {
        let s = keygen::setup(&mut rng, p, COUNT, THRESHOLD).unwrap();
        commitments.insert(p, s.commitment());
        secrets.insert(p, s);
    }
    // Every commitment carries a valid PoP: all keys are owned.
    let verified = PARTICIPANTS
        .into_iter()
        .map(|p| (p, keygen::verify_commitment(p, &commitments[&p]).unwrap()))
        .collect::<BTreeMap<_, _>>();
    let mut states = BTreeMap::new();
    let mut published = BTreeMap::new();
    for (p, s) in secrets {
        let (state, share) = keygen::generate_secret_shares(s, verified.clone()).unwrap();
        states.insert(p, state);
        published.insert(p, share);
    }
    (states, published)
}

#[test]
fn two_time_pad_on_every_pair_after_pop() {
    let (states, published) = ceremony();
    let mut pairs = 0;
    for x in PARTICIPANTS {
        for y in PARTICIPANTS {
            if x >= y {
                continue;
            }
            let c_xy: U256 = published[&x].f[slot(x, y)];
            let c_yx: U256 = published[&y].f[slot(y, x)];
            let f_xy = keygen::reveal_secret_share(&states[&x], y).unwrap();
            let f_yx = keygen::reveal_secret_share(&states[&y], x).unwrap();
            assert_eq!(c_xy ^ c_yx, f_xy ^ f_yx, "pair ({x}, {y}) is a two-time pad");
            pairs += 1;
        }
    }
    println!("two-time pad holds on {pairs} of 10 pairs");
    assert_eq!(pairs, 10);
}

#[test]
fn complaint_response_opens_plaintiffs_own_share() {
    let (states, published) = ceremony();
    let plaintiff = PARTICIPANTS[0];
    let mut opened = 0;
    for accused in PARTICIPANTS {
        if accused == plaintiff {
            continue;
        }
        // What `handle_key_gen_complained` publishes for the accused.
        let response = keygen::reveal_secret_share(&states[&accused], plaintiff).unwrap();
        // Any observer: pad from the accused's slot, then the other direction.
        let pad = response ^ published[&accused].f[slot(accused, plaintiff)];
        let recovered = published[&plaintiff].f[slot(plaintiff, accused)] ^ pad;
        let truth = keygen::reveal_secret_share(&states[&plaintiff], accused).unwrap();
        assert_eq!(recovered, truth, "the response to the complaint against {accused} opened f_P({accused})");
        opened += 1;
    }
    println!("4 complaint responses opened {opened} of the plaintiff's own shares");
    assert_eq!(opened, 4);
}
