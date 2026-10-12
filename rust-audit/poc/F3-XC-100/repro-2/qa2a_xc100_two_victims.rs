// ===========================================================================
// QA3-2A second reproduction of F3-XC-100 (independent of the primary QA).
//
// Route chosen DIFFERENT from the obvious one-victim case: the last committer
// cancels TWO honest verifying shares at once (threshold 3 leaves t-1 = 2 free
// coefficients, c[1] and c[2]), exactly the "up to t-1 victims" amplification
// the Critic noted. With the Aegis set (5 validators, threshold 3) the running
// honest validator ends its share round having seen only the two surviving
// honest shares; it excludes {attacker, victim1, victim2} and the restart set
// (2 remaining) is below min_participants(5) = 4, so the whole rollover is
// skipped.
//
// This module is appended to a copy of crates/validator/src/state/keygen.rs in
// the rv-core-b worktree, run with `cargo test -p validator --bins`, then the
// hunk is reverted (git checkout --). It is not part of the repository.
//
// What this executes end to end on the real validator code at 361aaf3:
//   1. The real FROST keygen (frost::keygen::setup / verify_commitment) accepts
//      the attacker's crafted last commitment (valid PoK of c[0], valid PoP of
//      q; only c[1]/c[2] rewritten).
//   2. The real frost::keygen::generate_secret_shares makes BOTH victims'
//      published verifying share y the identity point (0,0); the two survivors'
//      shares stay non-identity.
//   3. The real Transition state machine, driven through NewBlock / Resume /
//      KeyGenCommitted / KeyGenSecretShared / NewBlock, excludes
//      {attacker, victim1, victim2} at the share deadline and reaches
//      RolloverState::EpochSkipped.
//
// The on-chain rejection of y = (0,0) with Secp256k1.NotOnCurve (why the two
// victim shares never land, so they never enter `shared`) is L-crypto's
// executed E1 in state/run3/logs/L-crypto-identity-share.txt; here the two
// victim KeyGenSecretShared events are simply not delivered, as on chain.
// ===========================================================================
#[cfg(test)]
mod qa2a_xc100_two_victims {
    use super::*;
    use crate::{
        bindings,
        config::{Participant, ValidatorConfig},
        consensus::hashing::ConsensusDomain,
        frost::keygen as fk,
        service::{Event, Resume},
    };
    use alloy::primitives::{Address, U256, address};
    use frost_secp256k1::Identifier;
    use k256::{
        AffinePoint, EncodedPoint, ProjectivePoint, Scalar,
        elliptic_curve::{
            Group as _,
            sec1::{FromEncodedPoint as _, ToEncodedPoint as _},
        },
    };
    use safenet_core::{
        index::{EventBlock, EventLog},
        state::{Message, StateTransition as _},
    };

    // Aegis production: 5 genesis validators, threshold 3.
    const PARTICIPANTS: [Address; 5] = [
        address!("0x1000000000000000000000000000000000000001"), // H1 (running validator, survivor)
        address!("0x2000000000000000000000000000000000000002"), // H2 (survivor)
        address!("0x3000000000000000000000000000000000000003"), // V1 (victim 1)
        address!("0x4000000000000000000000000000000000000004"), // V2 (victim 2)
        address!("0x5000000000000000000000000000000000000005"), // A  (attacker, commits last)
    ];
    const H1: Address = PARTICIPANTS[0];
    const H2: Address = PARTICIPANTS[1];
    const V1: Address = PARTICIPANTS[2];
    const V2: Address = PARTICIPANTS[3];
    const ATTACKER: Address = PARTICIPANTS[4];

    fn transition(account: Address) -> Transition {
        let config = ValidatorConfig {
            consensus: address!("0xc855761D619f6002923507cE68B84d7689C2aa96"),
            staker: None,
            participants: PARTICIPANTS
                .iter()
                .map(|&address| Participant {
                    address,
                    active_from: 0,
                    active_before: None,
                })
                .collect(),
            oracles: BTreeSet::new(),
            genesis_salt: B256::ZERO,
            blocks_per_epoch: NonZeroU64::new(1440).unwrap(),
            key_gen_timeout: NonZeroU64::new(120).unwrap(),
            signing_timeout: NonZeroU64::new(6).unwrap(),
            oracle_timeout: NonZeroU64::new(24).unwrap(),
        };
        let genesis = group::participants_set(
            &config.participants,
            group::Epoch::Genesis {
                salt: config.genesis_salt,
            },
        )
        .unwrap();
        let consensus = ConsensusDomain::new(100, config.consensus);
        Transition {
            account,
            genesis,
            consensus,
            config,
        }
    }

    fn log(block: u64, index: u64, data: Event) -> Message<Event, Resume> {
        Message::Event(EventLog {
            block: EventBlock {
                number: block,
                timestamp: 0,
            },
            index,
            address: Address::ZERO,
            data,
        })
    }

    // --- secp256k1 point <-> ABI Point conversions (k256 directly, since the
    // frost::marshal module is private to the frost module). ---
    fn to_pp(p: &bindings::Point) -> ProjectivePoint {
        if p.x.is_zero() && p.y.is_zero() {
            return ProjectivePoint::IDENTITY;
        }
        let mut bytes = [0u8; 65];
        bytes[0] = 0x04;
        bytes[1..33].copy_from_slice(&p.x.to_be_bytes::<32>());
        bytes[33..65].copy_from_slice(&p.y.to_be_bytes::<32>());
        let ep = EncodedPoint::from_bytes(bytes).expect("uncompressed encoding");
        AffinePoint::from_encoded_point(&ep)
            .map(ProjectivePoint::from)
            .expect("point on curve")
    }
    fn from_pp(p: &ProjectivePoint) -> bindings::Point {
        if bool::from(p.is_identity()) {
            return bindings::Point {
                x: U256::ZERO,
                y: U256::ZERO,
            };
        }
        let ep = p.to_encoded_point(false);
        let b = ep.as_bytes();
        bindings::Point {
            x: U256::from_be_slice(&b[1..33]),
            y: U256::from_be_slice(&b[33..65]),
        }
    }
    fn xscalar(addr: Address) -> Scalar {
        Identifier::derive(addr.as_slice()).unwrap().to_scalar()
    }

    /// Crafts the attacker's last commitment so that the group verifying shares
    /// at both victims are the identity. Returns the per-participant ABI
    /// commitments (attacker's c[1]/c[2] rewritten) keyed by address.
    fn craft_two_victim_cancellation(
        secrets: &BTreeMap<Address, Secrets>,
    ) -> BTreeMap<Address, bindings::KeyGenCommitment> {
        let mut commitments: BTreeMap<Address, bindings::KeyGenCommitment> = secrets
            .iter()
            .map(|(a, s)| (*a, s.commitment()))
            .collect();

        let x1 = xscalar(V1);
        let x2 = xscalar(V2);

        // Fixed part of the group commitment polynomial, i.e. everything not
        // controlled by the attacker's c[1]/c[2]:
        //   S(x) = (sum over all c[0]) + (sum over non-attacker c[1]) * x
        //                              + (sum over non-attacker c[2]) * x^2
        let sum_c0: ProjectivePoint = commitments
            .values()
            .map(|c| to_pp(&c.c[0]))
            .sum();
        let hh1: ProjectivePoint = commitments
            .iter()
            .filter(|(a, _)| **a != ATTACKER)
            .map(|(_, c)| to_pp(&c.c[1]))
            .sum();
        let hh2: ProjectivePoint = commitments
            .iter()
            .filter(|(a, _)| **a != ATTACKER)
            .map(|(_, c)| to_pp(&c.c[2]))
            .sum();
        let s = |x: Scalar| sum_c0 + hh1 * x + hh2 * (x * x);
        let s1 = s(x1);
        let s2 = s(x2);

        // Solve  [x1 x1^2; x2 x2^2] [CA1; CA2] = [-S(x1); -S(x2)]  in point space.
        let det = x1 * x2 * x2 - x1 * x1 * x2; // x1*x2*(x2 - x1)
        let det_inv = det.invert().unwrap();
        let ca1 = (s1 * (-(x2 * x2)) + s2 * (x1 * x1)) * det_inv;
        let ca2 = (s1 * x2 + s2 * (-x1)) * det_inv;

        let attacker = commitments.get_mut(&ATTACKER).unwrap();
        attacker.c[1] = from_pp(&ca1);
        attacker.c[2] = from_pp(&ca2);
        commitments
    }

    fn describe(state: &State) -> String {
        match &state.rollover {
            RolloverState::CollectingCommitments {
                group,
                commitments,
                deadline,
                ..
            } => format!(
                "CollectingCommitments(group={}, commitments={}, deadline={deadline:?})",
                group.id(),
                commitments.len()
            ),
            RolloverState::CollectingShares {
                group,
                shared,
                deadline,
                ..
            } => format!(
                "CollectingShares(group={}, shared={:?}, deadline={deadline:?})",
                group.id(),
                shared.iter().collect::<Vec<_>>()
            ),
            RolloverState::CollectingConfirmations { group, .. } => {
                format!("CollectingConfirmations(group={})", group.id())
            }
            RolloverState::EpochStaged { next_epoch } => format!("EpochStaged({next_epoch})"),
            RolloverState::EpochSkipped { next_epoch } => format!("EpochSkipped({next_epoch})"),
            RolloverState::Halted => "Halted".to_owned(),
            _ => "other".to_owned(),
        }
    }

    #[test]
    fn qa2a_last_committer_cancels_two_victims_and_skips_the_rollover() {
        let mut rng = rand::thread_rng();

        // Real DKG setup for all five validators at threshold 3.
        let secrets: BTreeMap<Address, Secrets> = PARTICIPANTS
            .iter()
            .map(|&p| (p, fk::setup(&mut rng, p, 5, 3).unwrap()))
            .collect();
        assert_eq!(secrets[&ATTACKER].commitment().c.len(), 3, "degree-2 polynomial");

        // Attacker, committing last, cancels BOTH victims' verifying shares.
        let commitments = craft_two_victim_cancellation(&secrets);

        // The crafted last commitment still verifies (PoK of c[0], PoP of q).
        let verified: BTreeMap<Address, VerifiedCommitment> = commitments
            .iter()
            .map(|(a, c)| {
                (
                    *a,
                    fk::verify_commitment(*a, c).expect("crafted commitment still verifies"),
                )
            })
            .collect();
        println!("all 5 crafted commitments verify (PoK of c[0] and PoP of q)");

        // CRYPTO EFFECT: both victims, computing honestly, publish y = (0,0);
        // the two survivors publish a real point. The attacker withholds its
        // share (and could not produce a self-consistent one anyway: its real
        // polynomial no longer matches its crafted commitment), so it is left
        // out of this loop.
        let mut shares: BTreeMap<Address, bindings::KeyGenSecretShare> = BTreeMap::new();
        for &p in PARTICIPANTS.iter().filter(|&&p| p != ATTACKER) {
            let (_, share) =
                fk::generate_secret_shares(secrets[&p].clone(), verified.clone()).unwrap();
            if p == V1 || p == V2 {
                assert_eq!(
                    share.y,
                    bindings::Point::default(),
                    "victim {p} verifying share forced to identity"
                );
                println!("victim {p} share y = (0, 0)  [rejected on chain by NotOnCurve]");
            } else {
                assert_ne!(share.y, bindings::Point::default());
                println!("survivor {p} share y is a real curve point");
            }
            shares.insert(p, share);
        }

        // --- Drive the real validator state machine as survivor H1. ---
        let t = transition(H1);
        let state = State {
            rollover: RolloverState::EpochSkipped {
                next_epoch: NonZeroU64::new(1).unwrap(),
            },
            ..Default::default()
        };

        // Epoch 2's key generation starts at block 1440 (next_number(1440) = 2).
        let (state, commands) = t.apply_transition(state, Message::NewBlock(1440));
        let group_id = commands
            .iter()
            .find_map(|c| match c {
                Command::Effect(Effect::KeyGenSetup { group_id, .. }) => Some(*group_id),
                _ => None,
            })
            .expect("NewBlock(1440) emits Effect::KeyGenSetup");

        // Local setup resumes before any commitment lands (live order).
        let resume = Message::Resume(Resume::Setup {
            group_id,
            secrets: Box::new(secrets[&H1].clone()),
        });
        let (mut state, _) = t.apply_transition(state, resume);

        // All five commit in block 1441, attacker LAST.
        for (i, &p) in PARTICIPANTS.iter().enumerate() {
            let event = Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenCommitted(
                Coordinator::KeyGenCommitted {
                    gid: group_id,
                    participant: p,
                    commitment: commitments[&p].clone(),
                    committed: i + 1 == PARTICIPANTS.len(),
                },
            ));
            (state, _) = t.apply_transition(state, log(1441, i as u64, event));
        }
        let post_commit = describe(&state);
        println!("after commitments: {post_commit}");

        // Bite point: with the defect present, the commitment round completes
        // straight into CollectingShares for the poisoned group. Remediation
        // option 1 (detect an identity verifying share and restart excluding
        // the last committer) makes this assertion fail, because the validator
        // restarts into a fresh group (excluding the attacker) instead of
        // collecting shares for the poisoned one.
        match &state.rollover {
            RolloverState::CollectingShares { group, .. } => {
                assert_eq!(group.id(), group_id, "shares collected for the poisoned group");
                println!("poisoned group {group_id} proceeds to share collection (defect present)");
            }
            _ => panic!(
                "remediation fired: poisoned group did not reach share collection; state is {post_commit}"
            ),
        }

        // Only the two survivor shares (H1's own and H2's) land on chain and
        // pass verification. The two victim shares revert (NotOnCurve) and the
        // attacker withholds, so none of those three events are ever delivered.
        for (j, &p) in [H1, H2].iter().enumerate() {
            let event = Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenSecretShared(
                Coordinator::KeyGenSecretShared {
                    gid: group_id,
                    participant: p,
                    share: shares[&p].clone(),
                    shared: false,
                },
            ));
            state = t.apply_transition(state, log(1443 + j as u64, 0, event)).0;
            println!("after share from {p}: {}", describe(&state));
        }

        // At the share deadline (1441 + key_gen_timeout 120 = 1561) the round
        // times out.
        let (state, _) = t.apply_transition(state, Message::NewBlock(1561));
        println!("after NewBlock(1561) share deadline: {}", describe(&state));

        match state.rollover {
            RolloverState::EpochSkipped { next_epoch } => {
                assert_eq!(next_epoch.get(), 2);
                println!(
                    "RESULT: exclude {{attacker, V1, V2}} -> 2 of 5 remain < min_participants(5)=4 \
                     -> rollover_failure -> EpochSkipped(2)"
                );
            }
            other => panic!("expected EpochSkipped(2), got {other:?}"),
        }
    }
}
