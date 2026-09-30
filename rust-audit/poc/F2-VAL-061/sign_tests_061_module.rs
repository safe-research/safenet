
#[cfg(test)]
mod qa2_val_b_061 {
    //! QA2-VAL-B proof-of-concept for F2-VAL-061 (temporary; reverted after the
    //! run). Shows that a `Coordinator` event's *emitting address* is never
    //! consulted by the state machine (so any watched contract, e.g. an
    //! allow-listed oracle, can inject one), and that a malformed log with a
    //! watched `topic0` fails decoding (which is fatal for the whole batch).
    use super::*;
    use crate::{
        config::{Participant, ValidatorConfig},
        consensus::group,
        service::Event,
    };
    use alloy::sol_types::SolEvent as _;
    use safenet_core::{
        index::{EventLog, events::Events},
        state::{Message, StateTransition as _},
    };
    use std::num::NonZeroU64;
    use super::super::{Epoch, NonceState};

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
    fn group_of(addrs: &[Address], salt: B256) -> group::Group {
        let p: Vec<Participant> = addrs
            .iter()
            .map(|a| Participant { address: *a, active_from: 0, active_before: None })
            .collect();
        group::participants_set(&p, group::Epoch::Genesis { salt }).unwrap().group()
    }
    fn transition(me: Address) -> Transition {
        let genesis = group::participants_set(
            &[
                Participant { address: me, active_from: 0, active_before: None },
                Participant { address: Address::repeat_byte(0xee), active_from: 0, active_before: None },
            ],
            group::Epoch::Genesis { salt: B256::ZERO },
        )
        .unwrap();
        Transition { account: me, genesis, consensus: hashing::ConsensusDomain::new(1, Address::ZERO), config: cfg() }
    }
    fn state_with_pending(honest: &group::Group, ks: &Arc<KeyShare>, message: B256) -> State {
        let mut chunks = BTreeMap::new();
        chunks.insert(0u64, Some(B256::repeat_byte(0x5a)));
        let mut epochs = BTreeMap::new();
        epochs.insert(EpochId::Genesis, Epoch { group: honest.clone(), key_share: ks.clone(), nonces: NonceState { next_sequence: 0, chunks } });
        let signers: BTreeSet<Address> = honest.participants().clone();
        let mut signing = BTreeMap::new();
        signing.insert(
            message,
            SigningState::WaitingForRequest {
                key_share: ks.clone(),
                group_id: honest.id(),
                responsible: None,
                packet: Packet::EpochRollover {
                    active_epoch: EpochId::Genesis,
                    proposed_epoch: NonZeroU64::new(1).unwrap(),
                    rollover_block: 0,
                    group_id: honest.id(),
                    group_key: bindings::Point::default(),
                },
                signers,
                deadline: 1_000,
            },
        );
        State { epochs, signing, ..State::default() }
    }

    #[test]
    fn f2_val_061_coordinator_event_outcome_is_independent_of_emitting_address() {
        let me = Address::repeat_byte(0x11);
        let honest = group_of(&[Address::repeat_byte(1), Address::repeat_byte(2)], B256::ZERO);
        let ks = Arc::new(KeyShare::dummy());
        let message = B256::repeat_byte(0x77);
        let transition = transition(me);

        let real_coordinator = Address::repeat_byte(0xc0);
        let attacker_oracle = Address::repeat_byte(0xba);

        // The identical Sign event, delivered as if emitted by the real
        // coordinator vs. by an allow-listed (attacker-controlled) oracle.
        let sign = Coordinator::Sign {
            initiator: me,
            gid: honest.id(),
            message,
            sid: B256::repeat_byte(0x01),
            sequence: 0,
        };
        let deliver = |from: Address| {
            let log = EventLog {
                block: 100,
                index: 0,
                address: from,
                data: Event::Coordinator(Coordinator::CoordinatorEvents::Sign(sign.clone())),
            };
            transition.apply_transition(state_with_pending(&honest, &ks, message), Message::Event(log))
        };
        let (from_coordinator, cmds_c) = deliver(real_coordinator);
        let (from_oracle, cmds_o) = deliver(attacker_oracle);

        // Both advance the session identically: the emitting address is never
        // consulted for a Coordinator event, so a watched oracle contract can
        // inject genuine-looking protocol events.
        assert!(matches!(from_coordinator.signing.get(&message), Some(SigningState::CollectNonceCommitments { .. })));
        assert!(matches!(from_oracle.signing.get(&message), Some(SigningState::CollectNonceCommitments { .. })));
        assert_eq!(cmds_c.len(), cmds_o.len(), "identical command output regardless of emitter");
        assert_eq!(cmds_o.len(), 1, "the forged event drove a RevealNonceCommitments effect");
    }

    #[test]
    fn f2_val_061_malformed_log_with_watched_topic_fails_to_decode() {
        // A well-formed Sign log decodes.
        let sign = Coordinator::Sign {
            initiator: Address::repeat_byte(0x99),
            gid: B256::repeat_byte(0x42),
            message: B256::repeat_byte(0x77),
            sid: B256::repeat_byte(0x01),
            sequence: 7,
        };
        let log_data = sign.encode_log_data();
        let topics = log_data.topics();
        assert!(
            <Event as Events>::decode_log(topics, &log_data.data).is_some(),
            "a valid Sign log decodes into the watched event set"
        );

        // The SAME watched topic0 with a truncated data blob fails to decode.
        // In `decode_and_sort` this becomes a fatal `Error::DecodeLog` for the
        // whole batch, halting the indexer (driver retries the same block).
        assert!(
            <Event as Events>::decode_log(topics, &[0u8; 5]).is_none(),
            "a malformed log with a watched topic0 fails to decode (fatal for the batch)"
        );
    }
}
