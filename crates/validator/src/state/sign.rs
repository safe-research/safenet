use super::{Packet, SigningState, State, Transition};
use crate::{
    bindings::{self, Consensus, Coordinator, Oracle, SignNonces},
    consensus::hashing,
    frost::{self, preprocess::Nonces},
    merkle::MerkleRoot,
    service::{Action, Effect},
};
use alloy::{
    primitives::{Address, B256},
    sol_types::SolCall as _,
};
use safenet_core::state::{Command, Commands};
use std::collections::BTreeMap;

impl Transition {
    /// Handles a validator's own request to sign a packet.
    pub(super) fn handle_sign(
        &self,
        mut state: State,
        block: u64,
        event: &Coordinator::Sign,
    ) -> (State, Commands<State, Self>) {
        let mut commands = Vec::new();

        // Unconditionally top up the group's nonce stock, regardless of
        // whether this validator is one of the selected signers: the
        // sequence is shared by every participant and only ever advances, so
        // it can run past this validator's local nonces even for signing
        // ceremonies it was never asked to take part in. `epochs` is small
        // (a handful of entries at most in regular operation), so a linear
        // scan for the matching group is fine.
        if let Some(epoch) = state
            .epochs
            .values()
            .find(|epoch| epoch.group.id() == event.gid)
        {
            commands.push(Command::Effect(Effect::TopupNonces {
                group_id: event.gid,
                key_share: epoch.key_share.clone(),
                sequence: event.sequence,
            }));
        }

        match state.signing.remove(&event.message) {
            Some(SigningState::WaitingToDecline { deadline, .. }) => {
                commands.push(Command::Action(Action::SignDecline {
                    signature_id: event.sid,
                    expires_at: deadline,
                }));
            }
            Some(SigningState::WaitingForRequest {
                key_share,
                packet,
                signers,
                ..
            }) => match packet {
                Packet::OracleTransaction { oracle, .. } => {
                    let deadline = Some(block.saturating_add(self.config.oracle_timeout.get()));
                    state.signing.insert(
                        event.message,
                        SigningState::WaitingForOracle {
                            key_share,
                            oracle,
                            group_id: event.gid,
                            signature_id: event.sid,
                            sequence: event.sequence,
                            packet,
                            signers,
                            deadline,
                        },
                    );
                    state
                        .signature_id_to_message
                        .insert(event.sid, event.message);
                }
                Packet::Transaction { .. } | Packet::EpochRollover { .. } => {
                    let deadline = Some(block.saturating_add(self.config.signing_timeout.get()));
                    state.signing.insert(
                        event.message,
                        SigningState::CollectNonceCommitments {
                            key_share,
                            group_id: event.gid,
                            signature_id: event.sid,
                            sequence: event.sequence,
                            revealed: BTreeMap::new(),
                            packet,
                            signers,
                            deadline,
                        },
                    );
                    state
                        .signature_id_to_message
                        .insert(event.sid, event.message);
                    commands.push(Command::Effect(Effect::RevealNonceCommitments {
                        group_id: event.gid,
                        signature_id: event.sid,
                        message: event.message,
                        sequence: event.sequence,
                    }));
                }
            },
            Some(other) => {
                tracing::warn!(
                    message = %event.message,
                    signature_id = %event.sid,
                    "unexpected sign event for message",
                );
                state.signing.insert(event.message, other);
            }
            None => {
                tracing::debug!(
                    message = %event.message,
                    signature_id = %event.sid,
                    "not participating in message signing ceremony",
                );
            }
        }

        (state, commands)
    }

    /// Publishes this validator's revealed nonce commitment once the
    /// [`Effect::RevealNonceCommitments`] effect has produced it, entering
    /// [`SigningState::CollectNonceCommitments`]'s collection round.
    pub(super) fn handle_nonce_commitments(
        &self,
        state: State,
        signature_id: B256,
        message: B256,
        nonces: SignNonces,
        proof: Vec<B256>,
    ) -> (State, Commands<State, Self>) {
        let deadline = match state.signing.get(&message) {
            Some(SigningState::CollectNonceCommitments {
                signature_id: sid,
                deadline,
                ..
            }) if *sid == signature_id => *deadline,
            _ => return (state, Vec::new()),
        };

        (
            state,
            vec![Command::Action(Action::RevealNonceCommitments {
                signature_id,
                nonces,
                proof,
                expires_at: deadline,
            })],
        )
    }

    /// Resolves an oracle-backed signing round once its result lands:
    /// approved, this validator reveals its nonce commitment (as in
    /// [`handle_sign`](Self::handle_sign)'s live-request case); rejected, the
    /// session is simply dropped. A result for anything other than a tracked
    /// [`SigningState::WaitingForOracle`] round is ignored, as is one from an
    /// oracle contract other than the one the packet named.
    pub(super) fn handle_oracle_result(
        &self,
        mut state: State,
        block: u64,
        oracle: Address,
        event: &Oracle::OracleResult,
    ) -> (State, Commands<State, Self>) {
        match state.signing.remove(&event.requestId) {
            Some(SigningState::WaitingForOracle {
                key_share,
                oracle: expected,
                signature_id,
                packet,
                signers,
                group_id,
                sequence,
                ..
            }) if expected == oracle && event.approved => {
                let deadline = Some(block.saturating_add(self.config.signing_timeout.get()));
                state.signing.insert(
                    event.requestId,
                    SigningState::CollectNonceCommitments {
                        key_share,
                        group_id,
                        signature_id,
                        sequence,
                        revealed: BTreeMap::new(),
                        packet,
                        signers,
                        deadline,
                    },
                );

                (
                    state,
                    vec![Command::Effect(Effect::RevealNonceCommitments {
                        group_id,
                        signature_id,
                        message: event.requestId,
                        sequence,
                    })],
                )
            }
            Some(SigningState::WaitingForOracle {
                signature_id,
                oracle: expected,
                ..
            }) if expected == oracle && !event.approved => {
                // Rejected: drop the session, along with the signature id
                // index entry eagerly set when the round was opened.
                state.signature_id_to_message.remove(&signature_id);
                (state, Vec::new())
            }
            Some(other) => {
                tracing::warn!(
                    request_id = %event.requestId,
                    "unexpected oracle result for request",
                );
                state.signing.insert(event.requestId, other);
                (state, Vec::new())
            }
            None => (state, Vec::new()),
        }
    }

    /// Tracks a peer's revealed nonce commitment. Once every expected signer
    /// has revealed, enters [`SigningState::CollectSigningShares`] and
    /// dispatches the [`Effect::UseNonce`] effect to burn this validator's own
    /// nonce and produce a signature share from the now-complete set of
    /// revealed commitments.
    pub(super) fn handle_sign_revealed_nonces(
        &self,
        mut state: State,
        block: u64,
        event: &Coordinator::SignRevealedNonces,
    ) -> (State, Commands<State, Self>) {
        let Some(&message) = state.signature_id_to_message.get(&event.sid) else {
            return (state, Vec::new());
        };

        match state.signing.remove(&message) {
            Some(SigningState::CollectNonceCommitments {
                key_share,
                group_id,
                signature_id,
                sequence,
                mut revealed,
                packet,
                signers,
                deadline,
            }) => {
                match signers
                    .contains(&event.participant)
                    .then(|| frost::sign::verify_revealed_nonces(event.participant, &event.nonces))
                {
                    Some(Ok(nonces)) => {
                        revealed.insert(event.participant, nonces);
                    }
                    Some(Err(err)) => {
                        tracing::warn!(
                            signature_id = %signature_id,
                            participant = %event.participant,
                            %err,
                            "ignoring invalid revealed nonce commitment",
                        );
                    }
                    None => {
                        tracing::warn!(
                            signature_id = %signature_id,
                            participant = %event.participant,
                            signing_selection = ?signers,
                            "ignoring nonce commitment from participant not in signing selection",
                        );
                    }
                }

                if revealed.len() < signers.len() {
                    state.signing.insert(
                        message,
                        SigningState::CollectNonceCommitments {
                            key_share,
                            group_id,
                            signature_id,
                            sequence,
                            revealed,
                            packet,
                            signers,
                            deadline,
                        },
                    );
                    return (state, Vec::new());
                }

                let deadline = Some(block.saturating_add(self.config.signing_timeout.get()));
                state.signing.insert(
                    message,
                    SigningState::CollectSigningShares {
                        key_share,
                        group_id,
                        signature_id,
                        revealed,
                        selections: BTreeMap::new(),
                        packet,
                        signers,
                        deadline,
                    },
                );

                (
                    state,
                    vec![Command::Effect(Effect::UseNonce {
                        group_id,
                        message,
                        sequence,
                    })],
                )
            }
            Some(other) => {
                state.signing.insert(message, other);
                (state, Vec::new())
            }
            None => (state, Vec::new()),
        }
    }

    /// Publishes this validator's signature share once the
    /// [`Effect::UseNonce`] effect has produced it, attaching the packet's
    /// completion callback (`stageEpoch`/`attestTransaction`/
    /// `attestOracleTransaction`) so the group's completed signature carries
    /// out its onchain effect automatically.
    pub(super) fn handle_nonces(
        &self,
        state: State,
        message: B256,
        nonces: Nonces,
    ) -> (State, Commands<State, Self>) {
        let Some(SigningState::CollectSigningShares {
            key_share,
            signature_id,
            revealed,
            packet,
            deadline,
            ..
        }) = state.signing.get(&message)
        else {
            return (state, Vec::new());
        };

        let result = match frost::sign::signature_share(key_share, nonces, revealed, &message) {
            Ok(result) => result,
            Err(err) => {
                tracing::warn!(
                    %message,
                    %signature_id,
                    %err,
                    "failed to compute signature shares for signing ceremony"
                );
                return (state, Vec::new());
            }
        };

        let signature_id = *signature_id;
        let callback = packet.attestation_callback(self.config.consensus);
        let expires_at = *deadline;

        (
            state,
            vec![Command::Action(Action::SignShare {
                signature_id,
                selection: result.selection,
                share: result.share,
                proof: result.proof,
                callback,
                expires_at,
            })],
        )
    }

    /// Tracks a peer's published signature share against a tracked
    /// [`SigningState::CollectSigningShares`] round. A share for anything else
    /// (untracked, already completed, or a different round entirely) is
    /// ignored.
    pub(super) fn handle_sign_shared(
        &self,
        mut state: State,
        event: &Coordinator::SignShared,
    ) -> (State, Commands<State, Self>) {
        let Some(&message) = state.signature_id_to_message.get(&event.sid) else {
            return (state, Vec::new());
        };

        if let Some(SigningState::CollectSigningShares { selections, .. }) =
            state.signing.get_mut(&message)
        {
            let selection_root = MerkleRoot(event.selectionRoot);
            let selection = selections.entry(selection_root).or_default();

            // Note that we do not verify whether or not `participant` is part
            // of our `signers` list. The contract already verifies that they
            // are part of the group, and it would not be possible for them to
            // submit a share to the same selection root as we did (because of
            // the Merkle inclusion proof that is verified onchain).
            selection.shares_from.insert(event.participant);
            selection.last_signer = Some(event.participant);
        }

        (state, Vec::new())
    }

    /// Completes a tracked [`SigningState::CollectSigningShares`] round,
    /// entering [`SigningState::WaitingForAttestation`]. The participant
    /// responsible for submitting the attestation is the last one to have
    /// published a share against the completed selection root, or unknown
    /// (in which case every participant is responsible) if this validator
    /// never observed a share for that particular selection.
    pub(super) fn handle_sign_completed(
        &self,
        mut state: State,
        block: u64,
        event: &Coordinator::SignCompleted,
    ) -> (State, Commands<State, Self>) {
        let Some(&message) = state.signature_id_to_message.get(&event.sid) else {
            return (state, Vec::new());
        };

        match state.signing.remove(&message) {
            Some(SigningState::CollectSigningShares {
                signature_id,
                selections,
                packet,
                ..
            }) => {
                let responsible = selections
                    .get(&MerkleRoot(event.selectionRoot))
                    .and_then(|selection| selection.last_signer);
                if responsible.is_none() {
                    tracing::warn!(
                        %signature_id,
                        selection_root = %event.selectionRoot,
                        "signing ceremony completed under an unobserved selection",
                    );
                }

                let deadline = Some(block.saturating_add(self.config.signing_timeout.get()));
                state.signing.insert(
                    message,
                    SigningState::WaitingForAttestation {
                        signature_id,
                        responsible,
                        packet,
                        deadline,
                    },
                );
                (state, Vec::new())
            }
            Some(other) => {
                state.signing.insert(message, other);
                (state, Vec::new())
            }
            None => (state, Vec::new()),
        }
    }
}

impl Packet {
    /// Builds the callback invoked once this packet's group signature
    /// completes: `attestTransaction`/`attestOracleTransaction` calldata
    /// targeting the `Consensus` contract. The signature id argument is left
    /// as a zero placeholder - the `Consensus` contract fills it in itself
    /// when it invokes the callback from a completed `signShareWithCallback`.
    fn attestation_callback(&self, consensus: Address) -> bindings::Callback {
        let (epoch, oracle, transaction) = match self {
            Packet::Transaction { epoch, transaction } => (*epoch, None, transaction),
            Packet::OracleTransaction {
                epoch,
                oracle,
                transaction,
            } => (*epoch, Some(*oracle), transaction),
            Packet::EpochRollover {
                proposed_epoch,
                rollover_block,
                group_id,
                ..
            } => {
                return bindings::Callback {
                    target: consensus,
                    context: Consensus::stageEpochCall {
                        proposedEpoch: proposed_epoch.get(),
                        rolloverBlock: *rollover_block,
                        groupId: *group_id,
                        signatureId: B256::ZERO,
                    }
                    .abi_encode()
                    .into(),
                };
            }
        };

        let safe_tx_struct_hash = hashing::safe_tx_hash(transaction);
        let context = match oracle {
            None => Consensus::attestTransactionCall {
                epoch: epoch.raw_value(),
                chainId: transaction.chainId,
                safe: transaction.safe,
                safeTxStructHash: safe_tx_struct_hash,
                signatureId: B256::ZERO,
            }
            .abi_encode(),
            Some(oracle) => Consensus::attestOracleTransactionCall {
                epoch: epoch.raw_value(),
                oracle,
                chainId: transaction.chainId,
                safe: transaction.safe,
                safeTxStructHash: safe_tx_struct_hash,
                signatureId: B256::ZERO,
            }
            .abi_encode(),
        };
        bindings::Callback {
            target: consensus,
            context: context.into(),
        }
    }
}
