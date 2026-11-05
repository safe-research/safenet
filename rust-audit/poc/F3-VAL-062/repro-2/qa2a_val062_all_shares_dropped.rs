// ===========================================================================
// QA3-2A second reproduction of F3-VAL-062 (independent of the primary QA).
//
// Route chosen DIFFERENT from the obvious single-dropped-share case: the warp
// page that replays the epoch's key-generation start carries ALL FIVE peer
// secret-share events before the local Resume::Setup. Every share is dropped,
// so the restarted validator reaches its share deadline with shared = {} and
// excludes the whole group, skipping the epoch itself (the stronger single-
// validator outcome the finding's trigger step 4 and Critic window note
// describe). The live-order control in the same module shows the drop and the
// deadline drift (1680 vs 1561) both vanish when the setup resumes first.
//
// Appended to a copy of crates/validator/src/state/keygen.rs in the rv-core-b
// worktree, run with `cargo test -p validator --bins`, then reverted. Models
// the driver exactly: a warp page applies NewBlock and every log of up to 100
// blocks in one handle_update (crates/core/src/state/mod.rs:225-244), and
// effects resume only after that update returns (crates/core/src/driver.rs:
// 272-278), so the setup of the re-emitted Effect::KeyGenSetup is applied after
// the page's share logs.
// ===========================================================================
#[cfg(test)]
mod qa2a_val062_all_shares_dropped {
    use super::*;
    use crate::{
        bindings,
        config::{Participant, ValidatorConfig},
        consensus::hashing::ConsensusDomain,
        frost::keygen as fk,
        service::{Event, Resume},
    };
    use alloy::primitives::{Address, address};
    use safenet_core::{
        index::{EventBlock, EventLog},
        state::{Message, StateTransition as _},
    };

    const PARTICIPANTS: [Address; 5] = [
        address!("0x1000000000000000000000000000000000000001"), // me (restarting validator B)
        address!("0x2000000000000000000000000000000000000002"),
        address!("0x3000000000000000000000000000000000000003"),
        address!("0x4000000000000000000000000000000000000004"),
        address!("0x5000000000000000000000000000000000000005"),
    ];
    const ME: Address = PARTICIPANTS[0];

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

    struct Dkg {
        group_id: B256,
        secrets: BTreeMap<Address, Secrets>,
        shares: BTreeMap<Address, bindings::KeyGenSecretShare>,
    }

    /// NewBlock(1440) starts epoch 2's key generation (next_number(1440) = 2)
    /// and emits Effect::KeyGenSetup; here we build every honest participant's
    /// real commitment and share.
    fn start(t: &Transition) -> (State, Dkg) {
        let state = State {
            rollover: RolloverState::EpochSkipped {
                next_epoch: NonZeroU64::new(1).unwrap(),
            },
            ..Default::default()
        };
        let (state, commands) = t.apply_transition(state, Message::NewBlock(1440));
        let group_id = commands
            .iter()
            .find_map(|c| match c {
                Command::Effect(Effect::KeyGenSetup { group_id, .. }) => Some(*group_id),
                _ => None,
            })
            .expect("NewBlock(1440) emits Effect::KeyGenSetup");

        let mut rng = rand::thread_rng();
        let secrets: BTreeMap<Address, Secrets> = PARTICIPANTS
            .iter()
            .map(|&p| (p, fk::setup(&mut rng, p, 5, 3).unwrap()))
            .collect();
        let verified: BTreeMap<Address, VerifiedCommitment> = PARTICIPANTS
            .iter()
            .map(|&p| (p, fk::verify_commitment(p, &secrets[&p].commitment()).unwrap()))
            .collect();
        let shares = PARTICIPANTS
            .iter()
            .map(|&p| {
                let (_, share) =
                    fk::generate_secret_shares(secrets[&p].clone(), verified.clone()).unwrap();
                (p, share)
            })
            .collect();
        (state, Dkg { group_id, secrets, shares })
    }

    fn commit_all(t: &Transition, mut state: State, dkg: &Dkg, block: u64) -> State {
        for (i, &p) in PARTICIPANTS.iter().enumerate() {
            let event = Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenCommitted(
                Coordinator::KeyGenCommitted {
                    gid: dkg.group_id,
                    participant: p,
                    commitment: dkg.secrets[&p].commitment(),
                    committed: i + 1 == PARTICIPANTS.len(),
                },
            ));
            (state, _) = t.apply_transition(state, log(block, i as u64, event));
        }
        state
    }

    fn share(t: &Transition, state: State, dkg: &Dkg, from: Address, block: u64) -> State {
        let event = Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenSecretShared(
            Coordinator::KeyGenSecretShared {
                gid: dkg.group_id,
                participant: from,
                share: dkg.shares[&from].clone(),
                shared: false,
            },
        ));
        t.apply_transition(state, log(block, 0, event)).0
    }

    fn setup(t: &Transition, state: State, dkg: &Dkg, me: Address) -> State {
        let resume = Message::Resume(Resume::Setup {
            group_id: dkg.group_id,
            secrets: Box::new(dkg.secrets[&me].clone()),
        });
        t.apply_transition(state, resume).0
    }

    fn describe(state: &State) -> String {
        match &state.rollover {
            RolloverState::CollectingCommitments {
                commitments,
                deadline,
                secrets,
                ..
            } => format!(
                "CollectingCommitments(commitments={}, secrets={}, deadline={deadline:?})",
                commitments.len(),
                matches!(
                    secrets,
                    KeyGenCommitment::Participating { secrets: Some(_), .. }
                )
            ),
            RolloverState::CollectingShares { shared, deadline, .. } => {
                format!("CollectingShares(shared={}, deadline={deadline:?})", shared.len())
            }
            RolloverState::CollectingConfirmations { .. } => "CollectingConfirmations".to_owned(),
            RolloverState::EpochSkipped { next_epoch } => format!("EpochSkipped({next_epoch})"),
            RolloverState::Halted => "Halted".to_owned(),
            _ => "other".to_owned(),
        }
    }

    /// Restart warp order: NewBlock(1440), the five commitments (own included)
    /// and ALL FIVE shares are applied in one page, then the setup resumes.
    /// Every share is dropped, the restarted validator excludes the whole
    /// group at its (drifted) share deadline, and the epoch is skipped.
    #[test]
    fn qa2a_warp_drops_all_shares_and_skips_the_epoch() {
        let t = transition(ME);
        let (state, dkg) = start(&t);
        let state = commit_all(&t, state, &dkg, 1441);
        println!("warp: after 5 commitments: {}", describe(&state));

        // All five KeyGenSecretShared land in the same page, before the setup.
        let mut state = state;
        for (i, &p) in PARTICIPANTS.iter().enumerate() {
            state = share(&t, state, &dkg, p, 1443 + i as u64);
        }
        println!("warp: after ALL 5 shares (same page): {}", describe(&state));
        assert!(
            matches!(state.rollover, RolloverState::CollectingCommitments { .. }),
            "shares seen before setup are dropped; still collecting commitments"
        );

        // The local setup resumes last; the round finalizes with shared = {}.
        let state = setup(&t, state, &dkg, ME);
        println!("warp: after Resume::Setup: {}", describe(&state));
        let RolloverState::CollectingShares { shared, deadline, .. } = &state.rollover else {
            panic!("expected CollectingShares, got {}", describe(&state));
        };
        assert_eq!(shared.len(), 0, "every peer share was dropped");
        // Deadline drift: derived from the commitment deadline (1560) + 120,
        // not from the last commitment block (1441) + 120 = 1561.
        assert_eq!(*deadline, Some(1680), "share deadline drifts to 1680");

        // At the drifted share deadline the validator excludes the whole group.
        let (state, _) = t.apply_transition(state, Message::NewBlock(1680));
        println!("warp: after NewBlock(1680) share deadline: {}", describe(&state));
        match state.rollover {
            RolloverState::EpochSkipped { next_epoch } => {
                assert_eq!(next_epoch.get(), 2);
                println!(
                    "RESULT: shared={{}} -> exclude all 5 -> 0 remain < min_participants(5)=4 \
                     -> rollover_failure -> EpochSkipped(2)"
                );
            }
            other => panic!("expected EpochSkipped(2), got {other:?}"),
        }
    }

    /// Live order control: the setup resumes before the shares land. No share
    /// is dropped, the round reaches confirmation collection, and the share
    /// deadline is 1561 (no drift). This is the behaviour every remediation
    /// restores.
    #[test]
    fn qa2a_live_order_keeps_every_share() {
        let t = transition(ME);
        let (state, dkg) = start(&t);
        let state = setup(&t, state, &dkg, ME);
        let state = commit_all(&t, state, &dkg, 1441);
        let RolloverState::CollectingShares { shared, deadline, .. } = &state.rollover else {
            panic!("expected CollectingShares after commitments, got {}", describe(&state));
        };
        assert_eq!(shared.len(), 0);
        assert_eq!(*deadline, Some(1561), "live share deadline is 1561 (no drift)");

        let mut state = state;
        for (i, &p) in PARTICIPANTS.iter().enumerate() {
            state = share(&t, state, &dkg, p, 1443 + i as u64);
        }
        println!("live: after ALL 5 shares: {}", describe(&state));
        assert!(
            matches!(state.rollover, RolloverState::CollectingConfirmations { .. }),
            "live order keeps every share and reaches confirmation collection"
        );
    }
}
