use super::{Packet, SigningState, State, Transition};
use crate::{
    bindings::{self, Consensus, Coordinator, Oracle, SignNonces},
    consensus::{epoch::EpochId, hashing},
    frost::{self, preprocess::Nonces, sign::SigningNonces},
    merkle::MerkleRoot,
    service::{Action, Effect},
};
use alloy::{
    primitives::{Address, B256, keccak256},
    sol_types::SolCall as _,
};
use safenet_core::state::{Command, Commands};
use std::{
    collections::{BTreeMap, btree_map::Entry},
    mem,
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

        // Preprocessing still runs, so keep its signing sequence up to date
        // for it to prune its nonce chunks, even if signing no longer uses
        // them.
        if let Some(epoch) = state
            .epochs
            .values_mut()
            .find(|epoch| epoch.group.id() == event.gid)
        {
            epoch.nonces.observe(event.sequence);
        }

        match state.signing.remove(&event.message) {
            Some(SigningState::WaitingForRequest {
                key_share,
                group_id,
                packet,
                oracle_approved,
                signers,
                ..
            }) if group_id == event.gid => match packet {
                Packet::Transaction { oracle, .. } if !oracle_approved => {
                    let deadline = block.saturating_add(self.config.oracle_timeout.get());
                    tracing::info!(
                        message = %event.message,
                        signature_id = %event.sid,
                        group_id = %event.gid,
                        %oracle,
                        "signing request waiting for oracle result"
                    );
                    state.signing.insert(
                        event.message,
                        SigningState::WaitingForOracle {
                            key_share,
                            oracle,
                            group_id,
                            signature_id: event.sid,
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
                    tracing::info!(
                        message = %event.message,
                        signature_id = %event.sid,
                        group_id = %event.gid,
                        sequence = event.sequence,
                        "accepted signing request; committing nonces"
                    );
                    commands.push(Command::Effect(Effect::GenerateNonces {
                        group_id: event.gid,
                        signature_id: event.sid,
                        key_share: key_share.clone(),
                    }));
                    state.signing.insert(
                        event.message,
                        SigningState::CollectNonceCommitments {
                            key_share,
                            group_id: event.gid,
                            signature_id: event.sid,
                            revealed: BTreeMap::new(),
                            packet,
                            signers,
                            deadline,
                        },
                    );
                    state
                        .signature_id_to_message
                        .insert(event.sid, event.message);
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

    /// Publishes this validator's nonce commitments once the
    /// [`Effect::GenerateNonces`] effect has produced them, entering
    /// [`SigningState::CollectNonceCommitments`]'s collection round.
    pub(super) fn handle_nonce_commitments_new(
        &self,
        state: State,
        signature_id: B256,
        nonces: SignNonces,
    ) -> (State, Commands<State, Self>) {
        let deadline = match state
            .signature_id_to_message
            .get(&signature_id)
            .and_then(|message| state.signing.get(message))
        {
            Some(SigningState::CollectNonceCommitments {
                signature_id: sid,
                deadline,
                ..
            }) if *sid == signature_id => *deadline,
            _ => return (state, Vec::new()),
        };

        (
            state,
            vec![Command::Action(Action::CommitNonces {
                signature_id,
                nonces,
                expires_at: deadline,
            })],
        )
    }

    /// Resolves an oracle-backed signing round once its result lands:
    /// approved, this validator commits its nonces (as in
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
                ..
            }) if expected == oracle && event.approved => {
                let deadline = block.saturating_add(self.config.signing_timeout.get());
                tracing::info!(
                    request_id = %event.requestId,
                    signature_id = %signature_id,
                    %oracle,
                    "oracle approved transaction; committing nonces"
                );
                let effect = Effect::GenerateNonces {
                    group_id,
                    signature_id,
                    key_share: key_share.clone(),
                };
                state.signing.insert(
                    event.requestId,
                    SigningState::CollectNonceCommitments {
                        key_share,
                        group_id,
                        signature_id,
                        revealed: BTreeMap::new(),
                        packet,
                        signers,
                        deadline,
                    },
                );

                (state, vec![Command::Effect(effect)])
            }
            Some(SigningState::WaitingForOracle {
                signature_id,
                oracle: expected,
                ..
            }) if expected == oracle && !event.approved => {
                // Rejected: drop the session, along with the signature id
                // index entry eagerly set when the round was opened.
                tracing::info!(
                    request_id = %event.requestId,
                    signature_id = %signature_id,
                    %oracle,
                    "oracle rejected transaction; dropping signing ceremony"
                );
                state.signature_id_to_message.remove(&signature_id);
                (state, Vec::new())
            }
            Some(other) => {
                tracing::warn!(
                    request_id = %event.requestId,
                    %oracle,
                    "unexpected oracle result for request",
                );
                state.signing.insert(event.requestId, other);
                (state, Vec::new())
            }
            None => (state, Vec::new()),
        }
    }

    /// Tracks a peer's nonce commitment. Only a signer's first commitment for
    /// the ceremony counts, and later ones are ignored. Once every expected
    /// signer has committed, enters [`SigningState::CollectSigningShares`] and
    /// dispatches the [`Effect::UseNonceNEW`] effect to burn this validator's
    /// own nonces and produce a signature share from the now-complete set of
    /// commitments. If the round times out first, the signers that did commit
    /// continue without the others instead (see
    /// [`handle_signing_timeouts`](Self::handle_signing_timeouts)).
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
                mut revealed,
                packet,
                signers,
                deadline,
            }) => {
                match revealed.entry(event.participant) {
                    _ if !signers.contains(&event.participant) => {
                        tracing::warn!(
                            signature_id = %signature_id,
                            participant = %event.participant,
                            signing_selection = ?signers,
                            "ignoring nonce commitment from participant not in signing selection",
                        );
                    }
                    Entry::Occupied(_) => {
                        tracing::warn!(
                            signature_id = %signature_id,
                            participant = %event.participant,
                            "ignoring repeated nonce commitment from participant",
                        );
                    }
                    Entry::Vacant(entry) => {
                        match frost::sign::verify_revealed_nonces(event.participant, &event.nonces)
                        {
                            Ok(nonces) => {
                                entry.insert(nonces);
                            }
                            Err(err) => {
                                tracing::warn!(
                                    signature_id = %signature_id,
                                    participant = %event.participant,
                                    %err,
                                    "ignoring invalid nonce commitment",
                                );
                            }
                        }
                    }
                }

                if revealed.len() < signers.len() {
                    state.signing.insert(
                        message,
                        SigningState::CollectNonceCommitments {
                            key_share,
                            group_id,
                            signature_id,
                            revealed,
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
                    vec![Command::Effect(Effect::UseNonceNEW {
                        message,
                        signature_id,
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
    /// completion callback (`stageEpoch`/`attestTransaction`) so the
    /// group's completed signature carries out its onchain effect
    /// automatically.
    pub(super) fn handle_nonces(
        &self,
        state: State,
        message: B256,
        nonces: Box<Nonces>,
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

        let nonces = (*nonces).into();
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

    /// Publishes this validator's signature share once the
    /// [`Effect::UseNonceNEW`] effect has produced its nonces, attaching the
    /// packet's completion callback (`stageEpoch`/`attestTransaction`) so the
    /// group's completed signature carries out its onchain effect
    /// automatically.
    pub(super) fn handle_nonces_new(
        &self,
        state: State,
        message: B256,
        nonces: Box<SigningNonces>,
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

        let result = match frost::sign::signature_share(key_share, *nonces, revealed, &message) {
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
    /// (`EpochStaged`, `TransactionAttested`).
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

    /// Retries or drops every signing ceremony that has stalled past its
    /// deadline. Ports `signing/timeouts.ts`.
    pub(super) fn handle_signing_timeouts(
        &self,
        mut state: State,
        block: u64,
    ) -> (State, Commands<State, Self>) {
        let next_deadline = block.saturating_add(self.config.signing_timeout.get());
        let mut commands = Vec::new();

        for (message, signing) in &state.signing {
            if signing.deadline() <= block {
                tracing::warn!(
                    %message,
                    stage = signing.name(),
                    signature_id = ?signing.signature_id(),
                    deadline = signing.deadline(),
                    block,
                    "signing ceremony timed out"
                );
            }
        }

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
                revealed,
                packet,
                deadline,
                ..
            } if *deadline <= block => {
                // Instead of restarting the signing ceremony, continue it with
                // the signers that revealed their nonces before the deadline
                // (any later reveals are ignored). Make sure that there are
                // sufficient signers left (at least a group threshold of them)
                // and that we are part of the signing selection.
                if revealed.len() < key_share.group_threshold() as usize
                    || !revealed.contains_key(&self.account)
                {
                    state.signature_id_to_message.remove(signature_id);
                    return false;
                }

                tracing::info!(
                    %message,
                    %signature_id,
                    signing_selection = ?revealed.keys().collect::<Vec<_>>(),
                    "continuing signing ceremony with signers that revealed nonce commitments"
                );
                commands.push(Command::Effect(Effect::UseNonceNEW {
                    message: *message,
                    signature_id: *signature_id,
                }));
                *signing = SigningState::CollectSigningShares {
                    key_share: key_share.clone(),
                    group_id: *group_id,
                    signature_id: *signature_id,
                    signers: revealed.keys().copied().collect(),
                    revealed: mem::take(revealed),
                    selections: BTreeMap::new(),
                    packet: packet.clone(),
                    deadline: next_deadline,
                };
                true
            }
            SigningState::CollectSigningShares {
                key_share,
                group_id,
                signature_id,
                selections,
                packet,
                deadline,
                ..
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
                let signers = canonical_selection.shares_from;
                let last_signer = canonical_selection.last_signer;

                // The signature ID is no longer useful, unlink it.
                state.signature_id_to_message.remove(signature_id);

                // Ensure that there are sufficient signers left (at least a
                // group threshold of them) for restarting the ceremony and
                // that we are part of the signing selection.
                if signers.len() < key_share.group_threshold() as usize
                    || !signers.contains(&self.account)
                {
                    return false;
                }

                // We want to restart the signing process. By convention, the
                // last signer to participate is responsible for kicking it off.
                // If that is us, queue up an action for it.
                if last_signer == Some(self.account) {
                    commands.push(Command::Action(Action::Sign {
                        group_id: *group_id,
                        message: *message,
                        expires_at: next_deadline,
                    }));
                }

                // Ceremonies are only ever restarted once they have reached
                // the signature share round, which for oracle-backed packets
                // means that the oracle has already approved them.
                *signing = SigningState::WaitingForRequest {
                    key_share: key_share.clone(),
                    group_id: *group_id,
                    responsible: last_signer,
                    packet: packet.clone(),
                    oracle_approved: true,
                    signers,
                    deadline: next_deadline,
                };
                true
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
            _ => true,
        });

        (state, commands)
    }
}

impl SigningState {
    /// Returns a compact state name for diagnostics.
    fn name(&self) -> &'static str {
        match self {
            Self::WaitingForRequest { .. } => "waiting_for_request",
            Self::WaitingForOracle { .. } => "waiting_for_oracle",
            Self::CollectNonceCommitments { .. } => "collect_nonce_commitments",
            Self::CollectSigningShares { .. } => "collect_signing_shares",
            Self::WaitingForAttestation { .. } => "waiting_for_attestation",
        }
    }

    /// Returns the block deadline for the current state.
    fn deadline(&self) -> u64 {
        match self {
            Self::WaitingForRequest { deadline, .. }
            | Self::WaitingForOracle { deadline, .. }
            | Self::CollectNonceCommitments { deadline, .. }
            | Self::CollectSigningShares { deadline, .. }
            | Self::WaitingForAttestation { deadline, .. } => *deadline,
        }
    }

    /// Returns the known signature ID for a signing state, or `None` if none
    /// have been assigned yet.
    fn signature_id(&self) -> Option<B256> {
        match self {
            SigningState::WaitingForAttestation { signature_id, .. }
            | SigningState::WaitingForOracle { signature_id, .. }
            | SigningState::CollectNonceCommitments { signature_id, .. }
            | SigningState::CollectSigningShares { signature_id, .. } => Some(*signature_id),
            SigningState::WaitingForRequest { .. } => None,
        }
    }

    /// The packet to sign for a particular signing state.
    pub(super) fn packet(&self) -> &Packet {
        match self {
            SigningState::WaitingForRequest { packet, .. }
            | SigningState::WaitingForOracle { packet, .. }
            | SigningState::CollectNonceCommitments { packet, .. }
            | SigningState::CollectSigningShares { packet, .. }
            | SigningState::WaitingForAttestation { packet, .. } => packet,
        }
    }
}

impl Packet {
    /// The epoch whose group this packet is signed by.
    pub(super) fn epoch(&self) -> EpochId {
        match self {
            Packet::EpochRollover { active_epoch, .. } => *active_epoch,
            Packet::Transaction { epoch, .. } => *epoch,
        }
    }

    /// Builds the callback invoked once this packet's group signature
    /// completes: `stageEpoch`/`attestTransaction` calldata targeting
    /// the `Consensus` contract. The signature id argument is left as a zero
    /// placeholder - the `Consensus` contract fills it in itself when it
    /// invokes the callback from a completed `signShareWithCallback`.
    fn attestation_callback(&self, consensus: Address) -> bindings::Callback {
        let (epoch, oracle, oracle_data, transaction) = match self {
            Packet::Transaction {
                epoch,
                oracle,
                oracle_data,
                transaction,
            } => (*epoch, *oracle, oracle_data, transaction),
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
        let context = Consensus::attestTransactionCall {
            epoch: epoch.raw_value(),
            oracle,
            oracleDataHash: keccak256(oracle_data),
            chainId: transaction.chainId,
            safe: transaction.safe,
            safeTxStructHash: safe_tx_struct_hash,
            signatureId: B256::ZERO,
        }
        .abi_encode();
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
            Packet::Transaction {
                epoch,
                oracle,
                oracle_data,
                transaction,
            } => Action::AttestTransaction {
                epoch: *epoch,
                oracle: *oracle,
                oracle_data_hash: keccak256(oracle_data),
                chain_id: transaction.chainId,
                safe: transaction.safe,
                safe_tx_struct_hash: hashing::safe_tx_struct_hash(transaction),
                signature_id,
                expires_at,
            },
        }
    }
}
