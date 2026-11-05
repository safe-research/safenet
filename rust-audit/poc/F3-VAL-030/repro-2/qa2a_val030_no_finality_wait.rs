// ===========================================================================
// QA3-2A second reproduction of F3-VAL-030 (independent of the primary QA).
//
// Route chosen DIFFERENT from the obvious full two-validator Anvil reorg: this
// drives the REAL validator signing state machine (crates/validator/src/state/
// sign.rs) at Aegis parameters (5 validators, threshold 3, signing_timeout 6,
// oracle_timeout 24, max_reorg_depth 5) and shows the novel root cause (the
// finding's basis 1-4, Critic row 1): an approving OracleResult makes the
// validator commit to signing in the very block it sees the result, with a
// deadline of only block + signing_timeout and NO max_reorg_depth buffer, and
// within about two blocks it broadcasts a signShareWithCallback whose callback
// is attestTransaction. Nothing waits for the approval to become reorg-safe,
// so a reorg of depth 3 to 5 (inside max_reorg_depth) that removes the
// approving branch leaves the attestation-bearing share already out.
//
// The key material is real: a full 3-of-5 FROST DKG produces the key shares,
// the nonce commitments and this validator's signature share. The only setup
// is placing the validator in WaitingForOracle, the normal state every
// oracle-gated signing sits in before any result; the defect (acting at once
// on the approval) is exercised in full, with no mocked signatures.
//
// NOT executed here (cited in the finding, and the primary QA's Anvil route):
// the reorg rollback (a driver snapshot concern, basis 5) and the queue
// rebroadcast of the allocated share (basis 6), and the contracts accepting
// the attestation with no oracle gate (basis 7-9; the guard side is a settled
// design choice: oracle acceptance is validator policy). This PoC isolates
// the validator action that makes those
// downstream facts harmful.
//
// Appended to a copy of crates/validator/src/state/sign.rs in the rv-core-b
// worktree, run with `cargo test -p validator --bins`, then reverted.
// ===========================================================================
#[cfg(test)]
mod qa2a_val030_no_finality_wait {
    use super::*;
    use crate::{
        config::{Participant, ValidatorConfig},
        consensus::{group, hashing::ConsensusDomain},
        frost::keygen as fk,
        service::{Event, Resume},
    };
    use alloy::primitives::{Address, Bytes, address, keccak256};
    use safenet_core::{
        index::{EventBlock, EventLog},
        state::{Message, StateTransition as _},
    };
    use std::{collections::BTreeSet, num::NonZeroU64, sync::Arc};

    const PARTICIPANTS: [Address; 5] = [
        address!("0x1000000000000000000000000000000000000001"), // me
        address!("0x2000000000000000000000000000000000000002"), // signer 2
        address!("0x3000000000000000000000000000000000000003"), // signer 3
        address!("0x4000000000000000000000000000000000000004"),
        address!("0x5000000000000000000000000000000000000005"),
    ];
    const ME: Address = PARTICIPANTS[0];
    const CONSENSUS: Address = address!("0xc855761D619f6002923507cE68B84d7689C2aa96");
    const ORACLE: Address = address!("0x4F61B8832978e83b80D69551AEf07557DBE41d03");

    fn transition() -> Transition {
        let config = ValidatorConfig {
            consensus: CONSENSUS,
            staker: None,
            participants: PARTICIPANTS
                .iter()
                .map(|&address| Participant {
                    address,
                    active_from: 0,
                    active_before: None,
                })
                .collect(),
            oracles: BTreeSet::from([ORACLE]),
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
        Transition {
            account: ME,
            genesis,
            consensus: ConsensusDomain::new(100, CONSENSUS),
            config,
        }
    }

    /// A real 3-of-5 FROST DKG. Returns every participant's key share.
    fn run_dkg() -> std::collections::BTreeMap<Address, fk::KeyShare> {
        let mut rng = rand::thread_rng();
        let secrets: std::collections::BTreeMap<Address, _> = PARTICIPANTS
            .iter()
            .map(|&p| (p, fk::setup(&mut rng, p, 5, 3).unwrap()))
            .collect();
        let verified: std::collections::BTreeMap<Address, _> = PARTICIPANTS
            .iter()
            .map(|&p| (p, fk::verify_commitment(p, &secrets[&p].commitment()).unwrap()))
            .collect();
        let mut sharing = std::collections::BTreeMap::new();
        let mut shares = std::collections::BTreeMap::new();
        for (&p, s) in &secrets {
            let (st, share) = fk::generate_secret_shares(s.clone(), verified.clone()).unwrap();
            sharing.insert(p, st);
            shares.insert(p, share);
        }
        let mut key_shares = std::collections::BTreeMap::new();
        for (&p, st) in &sharing {
            let verified_shares = shares
                .iter()
                .map(|(&peer, share)| {
                    let enc = fk::verify_secret_share(st.group_commitments(), peer, share).unwrap();
                    let vs = fk::verify_encrypted_secret_share(st, peer, enc).unwrap();
                    (peer, vs)
                })
                .collect();
            key_shares.insert(p, fk::finalize(st.clone(), verified_shares).unwrap());
        }
        key_shares
    }

    fn safe_tx() -> bindings::SafeTransaction {
        bindings::SafeTransaction {
            chainId: alloy::primitives::U256::from(100u64),
            safe: address!("0x5afe000000000000000000000000000000000000"),
            to: address!("0xbeef000000000000000000000000000000000000"),
            value: alloy::primitives::U256::ZERO,
            data: Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef]),
            operation: bindings::Operation::CALL,
            safeTxGas: alloy::primitives::U256::ZERO,
            baseGas: alloy::primitives::U256::ZERO,
            gasPrice: alloy::primitives::U256::ZERO,
            gasToken: Address::ZERO,
            refundReceiver: Address::ZERO,
            nonce: alloy::primitives::U256::ZERO,
        }
    }

    fn event(block: u64, addr: Address, data: Event) -> Message<Event, Resume> {
        Message::Event(EventLog {
            block: EventBlock { number: block, timestamp: 0 },
            index: 0,
            address: addr,
            data,
        })
    }

    #[test]
    fn qa2a_approval_emits_attestation_share_without_reorg_safety() {
        let key_shares = run_dkg();
        let t = transition();

        let signers: BTreeSet<Address> = [PARTICIPANTS[0], PARTICIPANTS[1], PARTICIPANTS[2]]
            .into_iter()
            .collect();
        let request_id = keccak256("qa2a-val030-request");
        let signature_id = keccak256("qa2a-val030-sid");
        let group_id = keccak256("qa2a-val030-gid");

        // The validator sits in WaitingForOracle, the normal pre-result state
        // for an oracle-gated transaction. Its key share is real (from the DKG).
        let packet = Packet::Transaction {
            epoch: EpochId::Genesis,
            oracle: ORACLE,
            oracle_data: Bytes::new(),
            transaction: Box::new(safe_tx()),
        };
        let mut state = State::default();
        state.signing.insert(
            request_id,
            SigningState::WaitingForOracle {
                key_share: Arc::new(key_shares[&ME].clone()),
                oracle: ORACLE,
                group_id,
                signature_id,
                packet,
                signers: signers.clone(),
                deadline: 1000 + 24, // oracle_timeout
            },
        );
        state.signature_id_to_message.insert(signature_id, request_id);

        // BLOCK 1000: the oracle approves. The validator reacts in this block.
        let result_block = 1000u64;
        let (state, commands) = t.apply_transition(
            state,
            event(
                result_block,
                ORACLE,
                Event::Oracle(Oracle::OracleEvents::OracleResult(Oracle::OracleResult {
                    requestId: request_id,
                    proposer: ME,
                    result: Bytes::new(),
                    approved: true,
                })),
            ),
        );
        let generated = commands
            .iter()
            .any(|c| matches!(c, Command::Effect(Effect::GenerateNonces { .. })));
        assert!(generated, "approval emits GenerateNonces in the same block, no finality wait");
        // The new signing deadline is result_block + signing_timeout(6), with no
        // max_reorg_depth(5) buffer: the whole ceremony is due by block 1006.
        match state.signing.get(&request_id) {
            Some(SigningState::CollectNonceCommitments { deadline, .. }) => {
                assert_eq!(*deadline, result_block + 6);
                println!(
                    "block {result_block}: oracle approved -> GenerateNonces now; \
                     signing deadline {} (no +max_reorg_depth)",
                    result_block + 6
                );
            }
            other => panic!("expected CollectNonceCommitments, got {:?}", other.map(|s| s.name())),
        }

        // Each of the three signers commits its nonces (real FROST nonces).
        let mut rng = rand::thread_rng();
        let me_nonces = frost::sign::SigningNonces::generate(&key_shares[&ME], &mut rng);
        let me_commitments = me_nonces.commitments();
        let mut state = state;
        for (i, &p) in [PARTICIPANTS[0], PARTICIPANTS[1], PARTICIPANTS[2]].iter().enumerate() {
            let commitments = if p == ME {
                me_commitments.clone()
            } else {
                frost::sign::SigningNonces::generate(&key_shares[&p], &mut rng).commitments()
            };
            let (next, cmds) = t.apply_transition(
                state,
                event(
                    result_block + 1,
                    CONSENSUS,
                    Event::Coordinator(Coordinator::CoordinatorEvents::SignCommittedNonces(
                        Coordinator::SignCommittedNonces {
                            sid: signature_id,
                            participant: p,
                            nonces: commitments,
                        },
                    )),
                ),
            );
            state = next;
            if i + 1 == 3 {
                assert!(
                    cmds.iter().any(|c| matches!(c, Command::Effect(Effect::UseNonces { .. }))),
                    "threshold nonce commitments -> UseNonces at block {}",
                    result_block + 1
                );
            }
        }
        println!("block {}: threshold committed -> UseNonces (burn and share)", result_block + 1);

        // The UseNonces effect resumes with this validator's burned nonces,
        // producing the signature share and the signShareWithCallback.
        let (_, commands) = t.apply_transition(
            state,
            Message::Resume(Resume::Nonces {
                message: request_id,
                nonces: Box::new(me_nonces),
            }),
        );
        let sign_share = commands.iter().find_map(|c| match c {
            Command::Action(Action::SignShare { callback, expires_at, .. }) => {
                Some((callback.clone(), *expires_at))
            }
            _ => None,
        });
        let (callback, expires_at) = sign_share.expect("SignShare action emitted");
        assert_eq!(callback.target, CONSENSUS, "callback targets Consensus");
        let selector = <Consensus::attestTransactionCall as alloy::sol_types::SolCall>::SELECTOR;
        assert_eq!(&callback.context[..4], &selector[..], "callback is attestTransaction");
        assert!(
            expires_at <= result_block + 6 + 6,
            "share due well within max_reorg_depth of the approval"
        );
        println!(
            "SignShare broadcast with attestTransaction callback, expires_at {expires_at} \
             (approval at {result_block}, max_reorg_depth 5): a reorg removing the approval \
             still carries this attestation onto the canonical chain"
        );
    }
}
