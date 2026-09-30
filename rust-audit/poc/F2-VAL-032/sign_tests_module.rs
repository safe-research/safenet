
#[cfg(test)]
mod qa2_val_b {
    //! QA2-VAL-B proof-of-concept transitions (temporary; reverted after run).
    use super::*;
    use crate::{
        config::{Participant, ValidatorConfig},
        consensus::group,
        frost::preprocess::NonceChunk,
    };
    use std::num::NonZeroU64;
    use super::super::{Epoch, NonceIndex, NonceState};

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
        let participants: Vec<Participant> = addrs
            .iter()
            .map(|a| Participant { address: *a, active_from: 0, active_before: None })
            .collect();
        group::participants_set(&participants, group::Epoch::Genesis { salt })
            .unwrap()
            .group()
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
        Transition {
            account: me,
            genesis,
            consensus: hashing::ConsensusDomain::new(1, Address::ZERO),
            config: cfg(),
        }
    }

    fn rollover_packet(gid: B256) -> Packet {
        Packet::EpochRollover {
            active_epoch: EpochId::Genesis,
            proposed_epoch: NonZeroU64::new(1).unwrap(),
            rollover_block: 0,
            group_id: gid,
            group_key: bindings::Point::default(),
        }
    }

    fn waiting(ks: &Arc<KeyShare>, gid: B256, signers: BTreeSet<Address>) -> SigningState {
        SigningState::WaitingForRequest {
            key_share: ks.clone(),
            group_id: gid,
            responsible: None,
            packet: rollover_packet(gid),
            signers,
            deadline: 1_000,
        }
    }

    fn epoch_with_linked_chunk0(group: group::Group, ks: &Arc<KeyShare>) -> Epoch {
        let mut chunks = BTreeMap::new();
        chunks.insert(0u64, Some(B256::repeat_byte(0x5a)));
        Epoch { group, key_share: ks.clone(), nonces: NonceState { next_sequence: 0, chunks } }
    }

    // ---- F2-VAL-032 -------------------------------------------------------

    #[test]
    fn f2_val_032_foreign_group_sign_drops_pending_session() {
        let me = Address::repeat_byte(0x11);
        let honest = group_of(&[Address::repeat_byte(1), Address::repeat_byte(2)], B256::ZERO);
        let attacker =
            group_of(&[Address::repeat_byte(3), Address::repeat_byte(4)], B256::repeat_byte(0xaa));
        assert_ne!(honest.id(), attacker.id(), "the two groups must have distinct ids");

        let ks = Arc::new(KeyShare::dummy());
        let message = B256::repeat_byte(0x77);
        let signers: BTreeSet<Address> =
            [Address::repeat_byte(1), Address::repeat_byte(2)].into_iter().collect();

        let mut epochs = BTreeMap::new();
        epochs.insert(EpochId::Genesis, epoch_with_linked_chunk0(honest.clone(), &ks));
        let mut signing = BTreeMap::new();
        signing.insert(message, waiting(&ks, honest.id(), signers.clone()));
        let state = State { epochs, signing, ..State::default() };

        // An attacker who finalised its OWN 2-of-2 group emits Sign(G_att, m)
        // for the victim's rollover message. The validator does not track
        // G_att, so `observe` is never called and `nonce` is None.
        let event = Coordinator::Sign {
            initiator: Address::repeat_byte(0x99),
            gid: attacker.id(),
            message,
            sid: B256::repeat_byte(0x01),
            sequence: 0,
        };
        let transition = transition(me);
        let (after, commands) = transition.handle_sign(state, 100, &event);

        // BUG (F2-VAL-032): the pending honest session is GONE, even though
        // the Sign came from a group this validator does not track. The
        // correct behaviour is to leave it untouched.
        assert!(
            after.signing.get(&message).is_none(),
            "reproduction failed: the session survived the foreign-group Sign"
        );
        assert!(commands.is_empty(), "no commands expected on the drop arm");
    }

    #[test]
    fn f2_val_032_matching_group_sign_advances_session_control() {
        // Control: the SAME setup but the Sign comes from the tracked group,
        // so the session correctly advances (proving the drop above is due to
        // the group mismatch, not a generic failure).
        let me = Address::repeat_byte(0x11);
        let honest = group_of(&[Address::repeat_byte(1), Address::repeat_byte(2)], B256::ZERO);
        let ks = Arc::new(KeyShare::dummy());
        let message = B256::repeat_byte(0x77);
        let signers: BTreeSet<Address> =
            [Address::repeat_byte(1), Address::repeat_byte(2)].into_iter().collect();

        let mut epochs = BTreeMap::new();
        epochs.insert(EpochId::Genesis, epoch_with_linked_chunk0(honest.clone(), &ks));
        let mut signing = BTreeMap::new();
        signing.insert(message, waiting(&ks, honest.id(), signers.clone()));
        let state = State { epochs, signing, ..State::default() };

        let event = Coordinator::Sign {
            initiator: me,
            gid: honest.id(),
            message,
            sid: B256::repeat_byte(0x01),
            sequence: 0,
        };
        let transition = transition(me);
        let (after, commands) = transition.handle_sign(state, 100, &event);

        assert!(
            matches!(after.signing.get(&message), Some(SigningState::CollectNonceCommitments { .. })),
            "the tracked-group Sign should advance the session to nonce collection"
        );
        assert_eq!(commands.len(), 1, "one RevealNonceCommitments effect expected");
    }

    // ---- F2-VAL-033 -------------------------------------------------------

    #[test]
    fn f2_val_033_re_reveal_makes_re_revealer_the_last_signer() {
        let me = Address::repeat_byte(0x11);
        let a = Address::repeat_byte(0xaa);
        let b = Address::repeat_byte(0xbb);
        let m = Address::repeat_byte(0xcc);
        let signers: BTreeSet<Address> = [a, b, m].into_iter().collect();
        let sid = B256::repeat_byte(0x33);
        let message = B256::repeat_byte(0x77);
        let ks = Arc::new(KeyShare::dummy());

        // Two valid, independently-derived reveals; `nonces[0]` is reused for
        // M's first reveal and its re-reveal (identical leaf, as the contract
        // requires), `nonces[1]` is A's.
        let chunk = NonceChunk::with_size(2, &KeyShare::dummy(), &mut rand::thread_rng()).unwrap();

        let mut signing = BTreeMap::new();
        signing.insert(
            message,
            SigningState::CollectNonceCommitments {
                key_share: ks.clone(),
                group_id: B256::repeat_byte(0x42),
                signature_id: sid,
                nonce: NonceIndex { root: B256::ZERO, offset: 0 },
                revealed: BTreeMap::new(),
                last_signer: None,
                packet: rollover_packet(B256::repeat_byte(0x42)),
                signers: signers.clone(),
                deadline: 1_000,
            },
        );
        let mut sid_to_message = BTreeMap::new();
        sid_to_message.insert(sid, message);
        let state = State {
            signing,
            signature_id_to_message: sid_to_message,
            ..State::default()
        };
        let transition = transition(me);

        let reveal = |transition: &Transition, state: State, participant: Address, nonces| {
            let event = Coordinator::SignRevealedNonces { sid, participant, nonces };
            transition.handle_sign_revealed_nonces(state, 10, &event).0
        };
        let last_signer = |state: &State| match state.signing.get(&message) {
            Some(SigningState::CollectNonceCommitments { last_signer, revealed, .. }) => {
                (*last_signer, revealed.len())
            }
            _ => panic!("session should still be collecting nonce commitments"),
        };

        // M reveals, then A reveals, then M re-reveals its identical leaf.
        let state = reveal(&transition, state, m, chunk.nonces[0].reveal().0);
        assert_eq!(last_signer(&state), (Some(m), 1));
        let state = reveal(&transition, state, a, chunk.nonces[1].reveal().0);
        assert_eq!(last_signer(&state), (Some(a), 2));
        let state = reveal(&transition, state, m, chunk.nonces[0].reveal().0);

        // BUG (F2-VAL-033): the re-reveal overwrote M's entry and made M the
        // `last_signer` again, even though A revealed more recently and the
        // set of distinct revealers did not grow. On a timeout M becomes the
        // party responsible for the restart, costing one extra signing_timeout.
        assert_eq!(
            last_signer(&state),
            (Some(m), 2),
            "reproduction failed: the re-reveal did not reclaim last_signer"
        );
    }
}
