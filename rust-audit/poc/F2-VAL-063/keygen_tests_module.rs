
#[cfg(test)]
mod qa2_val_b {
    //! QA2-VAL-B proof-of-concept for F2-VAL-063 (temporary; reverted after the
    //! run). Shows that `start_key_gen` emits `KeyGenSetup` exactly once, and
    //! that a validator stranded in `CollectingCommitments { Participating {
    //! secrets: None } }` (a lost or failed setup) never has it re-emitted on a
    //! subsequent `NewBlock`.
    use super::*;
    use crate::{
        config::{Participant, ValidatorConfig},
        consensus::hashing::ConsensusDomain,
    };
    use safenet_core::state::{Message, StateTransition};

    fn cfg() -> ValidatorConfig {
        ValidatorConfig {
            consensus: Address::ZERO,
            staker: None,
            participants: Vec::new(),
            oracles: BTreeSet::new(),
            genesis_salt: B256::ZERO,
            blocks_per_epoch: NonZeroU64::new(1440).unwrap(),
            key_gen_timeout: NonZeroU64::new(120).unwrap(),
            signing_timeout: NonZeroU64::new(6).unwrap(),
            oracle_timeout: NonZeroU64::new(12).unwrap(),
        }
    }

    fn transition(me: Address, others: &[Address]) -> Transition {
        let mut parts = vec![Participant { address: me, active_from: 0, active_before: None }];
        parts.extend(
            others
                .iter()
                .map(|a| Participant { address: *a, active_from: 0, active_before: None }),
        );
        let genesis =
            group::participants_set(&parts, group::Epoch::Genesis { salt: B256::ZERO }).unwrap();
        Transition {
            account: me,
            genesis,
            consensus: ConsensusDomain::new(1, Address::ZERO),
            config: cfg(),
        }
    }

    fn count_keygen_setups(commands: &Commands<State, Transition>) -> usize {
        commands
            .iter()
            .filter(|c| matches!(c, Command::Effect(Effect::KeyGenSetup { .. })))
            .count()
    }

    #[test]
    fn f2_val_063_lost_key_gen_setup_is_never_re_issued() {
        let me = Address::repeat_byte(0x11);
        let peer = Address::repeat_byte(0x22);
        let transition = transition(me, &[peer]);

        // The DKG for epoch 2 opens at a block inside epoch-2's window (block
        // 1500 -> next_number = 2). `start_key_gen` writes the participating
        // commitment round with `secrets: None` and emits the ONE KeyGenSetup.
        let start_block = 1500u64;
        assert_eq!(epoch::next_number(start_block, cfg().blocks_per_epoch).get(), 2);
        let next_epoch = EpochId::Number { number: NonZeroU64::new(2).unwrap() };
        let participants = group::participants_set(
            &[
                Participant { address: me, active_from: 0, active_before: None },
                Participant { address: peer, active_from: 0, active_before: None },
            ],
            group::Epoch::Number {
                consensus: Address::ZERO,
                number: NonZeroU64::new(2).unwrap(),
                excluded: BTreeSet::new(),
            },
        )
        .unwrap();
        let deadline = Some(start_block + cfg().key_gen_timeout.get());
        let (state, commands) =
            transition.start_key_gen(State::default(), next_epoch, &participants, deadline);

        assert_eq!(count_keygen_setups(&commands), 1, "start_key_gen emits exactly one setup");
        assert!(
            matches!(
                &state.rollover,
                RolloverState::CollectingCommitments {
                    secrets: KeyGenCommitment::Participating { secrets: None, .. },
                    ..
                }
            ),
            "the round is left waiting for the setup effect to resume"
        );

        // The setup effect is LOST (aborted at shutdown, or its DB write
        // failed -> Resume::Noop). The state still has `secrets: None`. A later
        // NewBlock arrives, still inside epoch-2's window (not yet due) and well
        // before the keygen deadline.
        let next_block = start_block + 1;
        assert_eq!(epoch::next_number(next_block, cfg().blocks_per_epoch).get(), 2);
        let (state, commands) =
            transition.apply_transition(state, Message::NewBlock(next_block));

        // BUG (F2-VAL-063): nothing re-emits KeyGenSetup. The validator stays
        // stuck with `secrets: None` and will be excluded at the deadline.
        assert_eq!(
            count_keygen_setups(&commands),
            0,
            "reproduction failed: a KeyGenSetup was unexpectedly re-emitted"
        );
        assert!(
            matches!(
                &state.rollover,
                RolloverState::CollectingCommitments {
                    secrets: KeyGenCommitment::Participating { secrets: None, .. },
                    ..
                }
            ),
            "still stranded with no secrets and no way to recover before the deadline"
        );
    }
}
