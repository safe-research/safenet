#[cfg(test)]
mod poc_tests {
    use super::{KeyGenParticipation, RolloverState, State, Transition};
    use crate::{
        bindings::Coordinator,
        config::{Participant, ValidatorConfig},
        consensus::{epoch::EpochId, group, hashing::ConsensusDomain},
        frost::keygen,
        service::{Action, Event},
    };
    use alloy::primitives::{Address, B256, address};
    use safenet_core::{
        index::EventLog,
        state::{Command, Message, StateTransition},
    };
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::NonZeroU64,
    };

    const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
    const PEER1: Address = address!("70997970C51812dc3A010C7d01b50e0d17dc79C8");
    const PEER2: Address = address!("3C44CdDdB6a900fa2b585dd299e03d12FA4293BC");
    const CONSENSUS: Address = address!("0000000000000000000000000000000000000C0d");
    const BPE: u64 = 100;
    const KGT: u64 = 20;

    fn nz(n: u64) -> NonZeroU64 {
        NonZeroU64::new(n).unwrap()
    }

    fn participants() -> Vec<Participant> {
        [ME, PEER1, PEER2]
            .into_iter()
            .map(|address| Participant {
                address,
                active_from: 0,
                active_before: None,
            })
            .collect()
    }

    fn config() -> ValidatorConfig {
        ValidatorConfig {
            consensus: CONSENSUS,
            staker: None,
            participants: participants(),
            oracles: BTreeSet::new(),
            genesis_salt: B256::ZERO,
            blocks_per_epoch: nz(BPE),
            key_gen_timeout: nz(KGT),
            signing_timeout: nz(6),
            oracle_timeout: nz(12),
        }
    }

    fn transition() -> Transition {
        let genesis = group::participants_set(&participants(), group::Epoch::Genesis {
            salt: B256::ZERO,
        })
        .unwrap();
        Transition {
            account: ME,
            genesis,
            consensus: ConsensusDomain::new(100, CONSENSUS),
            config: config(),
        }
    }

    fn epoch_group(number: u64) -> group::Group {
        group::participants_set(&participants(), group::Epoch::Number {
            consensus: CONSENSUS,
            number: nz(number),
            excluded: BTreeSet::new(),
        })
        .unwrap()
        .group()
    }

    fn event_log(block: u64, data: Event) -> EventLog<Event> {
        EventLog {
            block,
            index: 0,
            address: CONSENSUS,
            data,
        }
    }

    fn committed(gid: B256, participant: Address, commitment: crate::bindings::KeyGenCommitment) -> Event {
        Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenCommitted(
            Coordinator::KeyGenCommitted {
                gid,
                participant,
                commitment,
                committed: true,
            },
        ))
    }

    // PoC for F2-VAL-003: a restart warp that covers the epoch boundary delivers
    // the peers' `KeyGenCommitted` logs with NO `NewBlock`, so the rollover clock
    // never ticks and the commitment is dropped -- whereas a delivered
    // `NewBlock(B)` ticks into `CollectingCommitments` and accepts the very same
    // event. The warp-emits-no-NewBlock half is confirmed in core (state/mod.rs).
    #[test]
    fn poc_f2_val_003_warp_boundary_drops_commitment() {
        let t = transition();
        let group2 = epoch_group(2);
        let (count, threshold) = group2.size();
        let gid2 = group2.id();

        // A peer's valid commitment for the epoch-2 group, seen at block 101.
        let mut rng = rand::thread_rng();
        let peer_secrets = keygen::setup(&mut rng, PEER1, count, threshold).unwrap();
        // `Event` is not `Clone`, so rebuild the identical peer log each time.
        let peer_event = || event_log(101, committed(gid2, PEER1, peer_secrets.commitment()));

        // (a) BUG: validator restarted across boundary 100. The warp delivered
        // the peer commitment as a log only; no NewBlock(100) ever ran, so the
        // rollover is still EpochStaged{1}. The event is silently dropped.
        let stale = State {
            rollover: RolloverState::EpochStaged { next_epoch: nz(1) },
            active_epoch: EpochId::Number { number: nz(1) },
            ..Default::default()
        };
        let (after_drop, cmds) = t.apply_transition(stale, Message::Event(peer_event()));
        assert!(cmds.is_empty(), "no commands on dropped event");
        assert!(
            matches!(after_drop.rollover, RolloverState::EpochStaged { .. }),
            "still EpochStaged; the peer commitment was dropped",
        );

        // (b) CONTRAST: had NewBlock(100) been delivered, the clock ticks into
        // CollectingCommitments{gid2} and the same event is accepted.
        let staged = State {
            rollover: RolloverState::EpochStaged { next_epoch: nz(1) },
            active_epoch: EpochId::Number { number: nz(1) },
            ..Default::default()
        };
        let (ticked, _) = t.apply_transition(staged, Message::NewBlock(100));
        let ticked_gid = match &ticked.rollover {
            RolloverState::CollectingCommitments { group, commitments, .. } => {
                assert!(commitments.is_empty(), "fresh round starts empty");
                group.id()
            }
            other => panic!("expected CollectingCommitments, got {other:?}"),
        };
        assert_eq!(ticked_gid, gid2, "clock tick opened the epoch-2 group");

        let (accepted, _) = t.apply_transition(ticked, Message::Event(peer_event()));
        match accepted.rollover {
            RolloverState::CollectingCommitments { commitments, .. } => {
                assert_eq!(commitments.len(), 1, "the same event is now accepted");
            }
            other => panic!("expected CollectingCommitments, got {other:?}"),
        }
    }

    // PoC for F2-VAL-006: a complaint against this validator is answered with the
    // plaintext share unconditionally (no plausibility check, `compromised`
    // ignored), and the plaintiff is not recorded anywhere -- only the accused is
    // -- so a false plaintiff pays nothing and can repeat it every ceremony.
    #[test]
    fn poc_f2_val_006_unconditional_unaccountable_complaint_response() {
        let t = transition();
        let group2 = epoch_group(2);
        let (count, threshold) = group2.size();
        let gid2 = group2.id();

        // Build a real sharing state for ME over the 3-party group.
        let mut rng = rand::thread_rng();
        let mut verified = BTreeMap::new();
        for who in [ME, PEER1, PEER2] {
            let secrets = keygen::setup(&mut rng, who, count, threshold).unwrap();
            verified.insert(who, keygen::verify_commitment(who, &secrets.commitment()).unwrap());
        }
        let me_secrets = keygen::setup(&mut rng, ME, count, threshold).unwrap();
        // Re-derive ME's verified commitment to match its secrets exactly.
        verified.insert(ME, keygen::verify_commitment(ME, &me_secrets.commitment()).unwrap());
        let (ss_me, _share) = keygen::generate_secret_shares(me_secrets, verified).unwrap();

        // The plaintext ME will (unconditionally) reveal for a false plaintiff.
        let expected = keygen::reveal_secret_share(&ss_me, PEER1).unwrap();

        let state = State {
            rollover: RolloverState::CollectingShares {
                next_epoch: EpochId::Number { number: nz(2) },
                group: group2,
                participation: KeyGenParticipation::Participating(ss_me),
                public_keys: BTreeMap::new(),
                shares: BTreeMap::new(),
                complaints: BTreeMap::new(),
                deadline: Some(200),
            },
            active_epoch: EpochId::Number { number: nz(1) },
            ..Default::default()
        };

        // PEER1 complains against ME (compromised flag false = no proof at all).
        let complaint = Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenComplained(
            Coordinator::KeyGenComplained {
                gid: gid2,
                plaintiff: PEER1,
                accused: ME,
                compromised: false,
            },
        ));
        let (after, cmds) = t.apply_transition(state, Message::Event(event_log(150, complaint)));

        // Exactly one response, revealing ME's plaintext share for the plaintiff.
        let responses: Vec<_> = cmds
            .iter()
            .filter_map(|c| match c {
                Command::Action(Action::KeyGenComplaintResponse { plaintiff, secret_share, .. }) => {
                    Some((*plaintiff, *secret_share))
                }
                _ => None,
            })
            .collect();
        assert_eq!(responses.len(), 1, "answered unconditionally");
        assert_eq!(responses[0], (PEER1, expected), "revealed the plaintext share");

        // No plaintiff accountability: complaints are keyed by accused only.
        match after.rollover {
            RolloverState::CollectingShares { complaints, .. } => {
                assert_eq!(complaints.get(&ME).map(|c| c.total), Some(1));
                assert!(
                    !complaints.contains_key(&PEER1),
                    "the plaintiff is never recorded",
                );
            }
            other => panic!("expected CollectingShares, got {other:?}"),
        }
    }
}
