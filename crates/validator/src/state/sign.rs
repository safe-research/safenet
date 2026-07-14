use super::{Packet, SigningState, State, Transition};
use crate::{
    bindings::{self, Consensus, Coordinator, Oracle, SignNonces},
    consensus::{epoch::EpochId, hashing},
    frost::{self, keygen::KeyShare, preprocess::Nonces},
    merkle::MerkleRoot,
    service::{Action, Effect},
};
use alloy::{
    primitives::{Address, B256},
    sol_types::SolCall as _,
};
use safenet_core::state::{Command, Commands};
use std::{
    collections::{BTreeMap, BTreeSet},
    mem,
    sync::Arc,
};

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
                group_id,
                packet,
                signers,
                ..
            }) if group_id == event.gid => match packet {
                Packet::OracleTransaction { oracle, .. } => {
                    let deadline = block.saturating_add(self.config.oracle_timeout.get());
                    state.signing.insert(
                        event.message,
                        SigningState::WaitingForOracle {
                            key_share,
                            oracle,
                            group_id,
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
                    let deadline = block.saturating_add(self.config.signing_timeout.get());
                    state.signing.insert(
                        event.message,
                        SigningState::CollectNonceCommitments {
                            key_share,
                            group_id: event.gid,
                            signature_id: event.sid,
                            sequence: event.sequence,
                            revealed: BTreeMap::new(),
                            last_signer: None,
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
                let deadline = block.saturating_add(self.config.signing_timeout.get());
                state.signing.insert(
                    event.requestId,
                    SigningState::CollectNonceCommitments {
                        key_share,
                        group_id,
                        signature_id,
                        sequence,
                        revealed: BTreeMap::new(),
                        last_signer: None,
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
                mut last_signer,
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
                        last_signer = Some(event.participant);
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
                            last_signer,
                            packet,
                            signers,
                            deadline,
                        },
                    );
                    return (state, Vec::new());
                }

                let deadline = block.saturating_add(self.config.signing_timeout.get());
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
    /// entering [`SigningState::WaitingForAttestation`]. The signature share
    /// that completed the ceremony should have submitted the attestation
    /// atomically through its callback. If that attestation does not arrive by
    /// the deadline, every validator submits the direct fallback instead.
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
                packet,
                ..
            }) => {
                let deadline = block.saturating_add(self.config.signing_timeout.get());
                state.signing.insert(
                    message,
                    SigningState::WaitingForAttestation {
                        signature_id,
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

    /// Handles a signature attestation, which can mean different things for
    /// different packets. Called by the individual attestations handlers
    /// (`EpochStaged`, `TransactionAttested`, `OracleTransactionAttested`).
    pub(super) fn handle_sign_attested(
        &self,
        mut state: State,
        signature_id: B256,
        message: B256,
    ) -> (State, Commands<State, Self>) {
        // Always clean up signing states when we observe attestations onchain
        // to prevent dangling signing references. This _should_ only happen in
        // case other validators diverge and produce an attestation under a
        // different signature ID, so this is purely defensive.
        let signing = state.signing.remove(&message);
        if let Some(signature_id) = signing.as_ref().and_then(|signing| signing.signature_id()) {
            state.signature_id_to_message.remove(&signature_id);
        }

        // In case we weren't in an expected state (either waiting for an
        // attestation or just observing but not participating in the signing
        // ceremony), log a warning, since this should never happen.
        match &signing {
            Some(SigningState::WaitingForAttestation { .. }) | None => {}
            Some(_) => tracing::warn!(
                %message,
                %signature_id,
                "received attestation on unexpected signing state"
            ),
        }

        (state, Vec::new())
    }

    /// Retries, declines, or drops every signing ceremony that has stalled
    /// past its deadline. Ports `signing/timeouts.ts`.
    pub(super) fn handle_signing_timeouts(
        &self,
        mut state: State,
        block: u64,
    ) -> (State, Commands<State, Self>) {
        let next_deadline = block.saturating_add(self.config.signing_timeout.get());
        let mut commands = Vec::new();

        // A helper for restarting a signing ceremony shared by timed out nonce
        // collection and signing share broadcasting.
        let restart_signing_ceremony =
            |signature_id_to_message: &mut BTreeMap<B256, B256>,
             commands: &mut Vec<Command<Action, Effect>>,
             key_share: Arc<KeyShare>,
             group_id: B256,
             signature_id: B256,
             signers: BTreeSet<Address>,
             message: B256,
             packet: Packet,
             last_signer: Option<Address>| {
                // The signature ID is no longer useful, unlink it.
                signature_id_to_message.remove(&signature_id);

                // Ensure that there are sufficient signers left (at least a
                // group threshold of them) for restarting the ceremony. and
                // that we are part of the signing selection.
                if signers.len() < key_share.group_threshold() as usize
                    || !signers.contains(&self.account)
                {
                    return None;
                }

                // We want to restart the signing process. By convention, the
                // last signer to participate is responsible for kicking if off.
                // If that is us, queue up an action for it.
                if last_signer == Some(self.account) {
                    commands.push(Command::Action(Action::Sign {
                        group_id,
                        message,
                        expires_at: next_deadline,
                    }));
                }
                Some(SigningState::WaitingForRequest {
                    key_share,
                    group_id,
                    responsible: last_signer,
                    packet,
                    signers,
                    deadline: next_deadline,
                })
            };

        state.signing.retain(|message, signing| match signing {
            SigningState::WaitingForRequest {
                key_share,
                group_id,
                responsible,
                signers,
                deadline,
                ..
            } if *deadline <= block => {
                let Some(previously_responsible) = responsible else {
                    // There is no one responsible, or the whole signing
                    // selection already tried to recover.
                    return false;
                };

                // In case the responsible party is a signer, remove them from
                // the signing selection. Make sure that we have sufficient
                // signers to continue and that we are still included.
                signers.remove(previously_responsible);
                if signers.len() < key_share.group_threshold() as usize
                    || !signers.contains(&self.account)
                {
                    return false;
                }

                // We need to restart the signing ceremony, make everyone
                // responsible. This is a bit heavy handed, but otherwise there
                // is no one that we can definitively say is responsible for
                // doing this (the previously `responsible` party failed in
                // their duties and are not part of the signing selection
                // anymore with no incentive to execute the action).
                *responsible = None;
                *deadline = next_deadline;
                commands.push(Command::Action(Action::Sign {
                    group_id: *group_id,
                    message: *message,
                    expires_at: next_deadline,
                }));
                true
            }
            SigningState::WaitingForOracle {
                key_share,
                oracle,
                group_id,
                signature_id,
                sequence,
                packet,
                signers,
                deadline,
            } if *deadline <= block => {
                // The oracle did not respond in time, drop the signing.
                state.signature_id_to_message.remove(signature_id);
                false
            }
            SigningState::CollectNonceCommitments {
                key_share,
                group_id,
                signature_id,
                sequence,
                revealed,
                last_signer,
                packet,
                signers,
                deadline,
            } if *deadline <= block => {
                // The remaining signers are all the ones that revealed nonces.
                signers.retain(|signer| revealed.contains_key(signer));

                if let Some(new_state) = restart_signing_ceremony(
                    &mut state.signature_id_to_message,
                    &mut commands,
                    key_share.clone(),
                    *group_id,
                    *signature_id,
                    mem::take(signers),
                    *message,
                    packet.clone(),
                    *last_signer,
                ) {
                    *signing = new_state;
                    true
                } else {
                    false
                }
            }
            SigningState::CollectSigningShares {
                key_share,
                group_id,
                signature_id,
                revealed,
                selections,
                packet,
                signers,
                deadline,
            } if *deadline <= block => {
                // Select the largest section that is at least as large as the
                // group threshold. This is necessarily unique because the
                // threshold is strictly larger than half the group size. If
                // none exist, then we do not have enough signers that agree to
                // restart the ceremony anyway.
                let canonical_selection = mem::take(selections)
                    .into_values()
                    .filter(|selection| {
                        selection.shares_from.len() >= key_share.group_threshold() as usize
                    })
                    .max_by_key(|selection| selection.shares_from.len())
                    .unwrap_or_default();

                if let Some(new_state) = restart_signing_ceremony(
                    &mut state.signature_id_to_message,
                    &mut commands,
                    key_share.clone(),
                    *group_id,
                    *signature_id,
                    canonical_selection.shares_from,
                    *message,
                    packet.clone(),
                    canonical_selection.last_signer,
                ) {
                    *signing = new_state;
                    true
                } else {
                    false
                }
            }
            SigningState::WaitingForAttestation {
                signature_id,
                packet,
                deadline,
            } if *deadline <= block => {
                // Build the fallback action for the packet. Note that we make
                // everyone responsible for getting this onchain, as the party
                // that was theoretically responsible for it in the first place
                // is clearly no interested.
                commands.push(Command::Action(
                    packet.attestation_action(*signature_id, next_deadline),
                ));

                // We will not retry to get the attestation onchain again, so if
                // this fails, then there is something seriously wrong. In any
                // case, we want to clean up.
                state.signature_id_to_message.remove(signature_id);
                false
            }
            SigningState::WaitingToDecline { packet, deadline } if *deadline <= block => {
                // Declining is indicative anyway, its not a big deal if we did
                // not do it. Just clean up the signing ceremony.
                false
            }
            _ => true,
        });

        (state, commands)
    }
}

impl SigningState {
    /// Returns the known signature ID for a signing state, or `None` if none
    /// have been assigned yet.
    fn signature_id(&self) -> Option<B256> {
        match self {
            SigningState::WaitingForAttestation { signature_id, .. }
            | SigningState::WaitingForOracle { signature_id, .. }
            | SigningState::CollectNonceCommitments { signature_id, .. }
            | SigningState::CollectSigningShares { signature_id, .. } => Some(*signature_id),
            SigningState::WaitingForRequest { .. } | SigningState::WaitingToDecline { .. } => None,
        }
    }

    /// The packet to sign for a particular signing state.
    pub(super) fn packet(&self) -> &Packet {
        match self {
            SigningState::WaitingForRequest { packet, .. }
            | SigningState::WaitingForOracle { packet, .. }
            | SigningState::CollectNonceCommitments { packet, .. }
            | SigningState::CollectSigningShares { packet, .. }
            | SigningState::WaitingForAttestation { packet, .. }
            | SigningState::WaitingToDecline { packet, .. } => packet,
        }
    }
}

impl Packet {
    /// The epoch whose group this packet is signed by.
    pub(super) fn epoch(&self) -> EpochId {
        match self {
            Packet::EpochRollover { active_epoch, .. } => *active_epoch,
            Packet::Transaction { epoch, .. } | Packet::OracleTransaction { epoch, .. } => *epoch,
        }
    }

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

        let safe_tx_struct_hash = hashing::safe_tx_struct_hash(transaction);
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

    /// Builds the fallback action to directly submit a completed attestation,
    /// for when the automatic `signShareWithCallback` submission did not land
    /// in time.
    fn attestation_action(&self, signature_id: B256, expires_at: u64) -> Action {
        match self {
            Packet::EpochRollover {
                proposed_epoch,
                rollover_block,
                group_id,
                ..
            } => Action::StageEpoch {
                proposed_epoch: *proposed_epoch,
                rollover_block: *rollover_block,
                group_id: *group_id,
                signature_id,
                expires_at,
            },
            Packet::Transaction { epoch, transaction } => Action::AttestTransaction {
                epoch: *epoch,
                chain_id: transaction.chainId,
                safe: transaction.safe,
                safe_tx_struct_hash: hashing::safe_tx_struct_hash(transaction),
                signature_id,
                expires_at,
            },
            Packet::OracleTransaction {
                epoch,
                oracle,
                transaction,
            } => Action::AttestOracleTransaction {
                epoch: *epoch,
                oracle: *oracle,
                chain_id: transaction.chainId,
                safe: transaction.safe,
                safe_tx_struct_hash: hashing::safe_tx_struct_hash(transaction),
                signature_id,
                expires_at,
            },
        }
    }
}
