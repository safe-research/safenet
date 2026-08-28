use super::{
    ConfirmationDeadlines, Epoch, KeyGenCommitment, KeyGenConfirmation, KeyGenParticipation,
    NonceState, Packet, RolloverState, SigningState, State, Transition,
};
use crate::{
    bindings::{Consensus, Coordinator},
    consensus::{
        epoch::{self, EpochId},
        group::{self, Group, ParticipantSet},
    },
    frost::{
        self,
        keygen::{
            GroupCommitments, KeyShare, Secrets, SharingState, VerifiedCommitment, VerifiedShare,
        },
    },
    service::{Action, Effect},
};
use alloy::{
    primitives::{Address, B256},
    sol_types::SolValue as _,
};
use safenet_core::state::{Command, Commands};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    iter, mem,
    num::NonZeroU64,
    sync::Arc,
};

impl Transition {
    /// Joins the genesis key generation once its group is created onchain.
    ///
    /// The genesis group is bootstrapped by an external `keyGen` call; each
    /// validator reacts to the resulting `KeyGen` event by entering commitment
    /// collection for the group it derives from its own configuration.
    ///
    /// Other `KeyGen` events are ignored by the validators, as regular epoch
    /// rotation is triggered on new blocks.
    pub(super) fn handle_genesis_key_gen(
        &self,
        state: State,
        event: &Coordinator::KeyGen,
    ) -> (State, Commands<State, Self>) {
        let genesis = self.genesis.group();
        if !matches!(state.rollover, RolloverState::WaitingForGenesis) || event.gid != genesis.id()
        {
            return (state, Vec::new());
        }

        // The genesis group generation is not subject to a rollover deadline.
        self.start_key_gen(state, EpochId::Genesis, &self.genesis, None)
    }

    /// Publishes the key gen commitment once the [`Effect::KeyGenSetup`]
    /// effect has produced it.
    ///
    /// Peer commitments are already being collected at this point, so this
    /// only fills in the secrets the round was missing - unless every
    /// commitment has already been collected, in which case the commitment
    /// round finalizes right away. This can happen in reorg situations where
    /// the blocks with the commitments (including the validator's own
    /// commitments) are replayed before the effect completes.
    pub(super) fn handle_key_gen_setup(
        &self,
        state: State,
        group_id: B256,
        secrets: Box<Secrets>,
    ) -> (State, Commands<State, Self>) {
        match state.rollover {
            RolloverState::CollectingCommitments {
                next_epoch,
                group,
                secrets:
                    KeyGenCommitment::Participating {
                        poap,
                        secrets: None,
                    },
                commitments,
                deadline,
            } if group_id == group.id() => {
                let (count, _) = group.size();
                tracing::debug!(
                    ?next_epoch,
                    %group_id,
                    ?deadline,
                    "key generation setup completed"
                );

                // A reorg can replay this validator's own commitment before
                // the setup that produced it resumes. The commitment is then
                // already onchain and must not be published a second time.
                let commands = if commitments.contains_key(&self.account) {
                    Vec::new()
                } else {
                    let (participants, count, threshold, context) = group.parameters();
                    vec![Command::Action(Action::KeyGenAndCommit {
                        participants,
                        count,
                        threshold,
                        context,
                        poap: poap.clone(),
                        commitment: secrets.commitment(),
                        expires_at: deadline,
                    })]
                };

                if commitments.len() as u16 != count {
                    // Commitments are still being collected, so stay in the
                    // same state with the secrets filled in.
                    return (
                        State {
                            rollover: RolloverState::CollectingCommitments {
                                next_epoch,
                                group,
                                secrets: KeyGenCommitment::Participating {
                                    poap,
                                    secrets: Some(secrets),
                                },
                                commitments,
                                deadline,
                            },
                            ..state
                        },
                        commands,
                    );
                }

                // Every participant had already committed, so the commitment
                // round closes as soon as the setup lands.
                let deadline = deadline
                    .map(|deadline| deadline.saturating_add(self.config.key_gen_timeout.get()));
                let (rollover, finalize_commands) = self.finalize_key_gen_commitments(
                    next_epoch,
                    group,
                    Some(secrets),
                    commitments,
                    deadline,
                );

                (
                    State { rollover, ..state },
                    [commands, finalize_commands].concat(),
                )
            }
            _ => (state, Vec::new()),
        }
    }

    /// Registers a peer's key generation commitment. Once every participant
    /// has committed, moves the group into secret-share collection: kicking
    /// off the [`Effect::DkgShares`] effect if this validator is participating,
    /// or straight into [`RolloverState::CollectingShares`] otherwise.
    pub(super) fn handle_key_gen_committed(
        &self,
        state: State,
        block: u64,
        event: &Coordinator::KeyGenCommitted,
    ) -> (State, Commands<State, Self>) {
        match state.rollover {
            RolloverState::CollectingCommitments {
                next_epoch,
                group,
                secrets,
                mut commitments,
                deadline,
            } if group.id() == event.gid => {
                let (count, _) = group.size();

                // Only consider valid commitments; invalid ones are ignored,
                // the participant will be removed from the group on timeout.
                match frost::keygen::verify_commitment(event.participant, &event.commitment) {
                    Ok(commitment) => {
                        commitments.insert(event.participant, commitment);
                        tracing::debug!(
                            ?next_epoch,
                            group_id = %event.gid,
                            participant = %event.participant,
                            received = commitments.len(),
                            expected = count,
                            block,
                            ?deadline,
                            "accepted key generation commitment"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(
                            %err,
                            participant = %event.participant,
                            "invalid key gen commitment"
                        );
                    }
                };

                if commitments.len() as u16 != count {
                    // We are still missing commitments, so stay in the same
                    // collecting state with the (possibly) updated commitments
                    // map.
                    return (
                        State {
                            rollover: RolloverState::CollectingCommitments {
                                next_epoch,
                                group,
                                secrets,
                                commitments,
                                deadline,
                            },
                            ..state
                        },
                        Vec::new(),
                    );
                }

                // Every participant has committed, but this validator's own
                // key generation setup may still be outstanding, in which case
                // its secret shares cannot be computed yet and the round is
                // instead finalized by `handle_key_gen_setup` once it resumes.
                let secrets = match secrets {
                    KeyGenCommitment::Observing => None,
                    KeyGenCommitment::Participating {
                        secrets: Some(secrets),
                        ..
                    } => Some(secrets),
                    secrets @ KeyGenCommitment::Participating { secrets: None, .. } => {
                        tracing::debug!(
                            ?next_epoch,
                            group_id = %event.gid,
                            "all key generation commitments received; waiting for setup"
                        );
                        return (
                            State {
                                rollover: RolloverState::CollectingCommitments {
                                    next_epoch,
                                    group,
                                    secrets,
                                    commitments,
                                    deadline,
                                },
                                ..state
                            },
                            Vec::new(),
                        );
                    }
                };

                // Every participant has committed, so a fresh round starts:
                // push the deadline forward from the current block. Genesis
                // is not subject to a rollover deadline, so `None` stays
                // `None`.
                let deadline =
                    deadline.map(|_| block.saturating_add(self.config.key_gen_timeout.get()));
                let (rollover, commands) = self.finalize_key_gen_commitments(
                    next_epoch,
                    group,
                    secrets,
                    commitments,
                    deadline,
                );

                (State { rollover, ..state }, commands)
            }
            _ => (state, Vec::new()),
        }
    }

    /// Registers a peer's key generation secret share, verified against its
    /// earlier commitment. An invalid share raises a [`Action::KeyGenComplain`]
    /// against its sender; share collection still completes once every
    /// participant has submitted one (valid or not), no different from a valid
    /// share, as invalid shares are resolved through the complaint flow. Once
    /// every share has been submitted, moves the group into confirmation
    /// collection, finalizing this validator's key share and emitting
    /// [`Action::KeyGenConfirm`] if every share it received was valid.
    pub(super) fn handle_key_gen_secret_shared(
        &self,
        state: State,
        block: u64,
        event: &Coordinator::KeyGenSecretShared,
    ) -> (State, Commands<State, Self>) {
        match state.rollover {
            RolloverState::CollectingShares {
                next_epoch,
                group,
                participation,
                mut public_keys,
                mut shares,
                complaints,
                deadline,
            } if group.id() == event.gid => {
                let (count, _) = group.size();
                let mut commands = Vec::new();

                // Only consider valid shared secrets (public key share that
                // matches their commitments and correct number of secret
                // shares); invalid ones are ignored, the participant will be
                // removed from the group on timeout.
                let encrypted_shares = match frost::keygen::verify_secret_share(
                    participation.group_commitments(),
                    event.participant,
                    &event.share,
                ) {
                    Ok((public_key, encrypted_shares)) => {
                        public_keys.insert(event.participant, public_key);
                        Some(encrypted_shares)
                    }
                    Err(err) => {
                        tracing::warn!(
                            %err,
                            participant = %event.participant,
                            "invalid key gen secret share"
                        );
                        None
                    }
                };

                // If we are participating, also verify the encrypted key shares
                // against our sharing state, and emit a complaint if required.
                if let (KeyGenParticipation::Participating(sharing_state), Some(encrypted_shares)) =
                    (&participation, encrypted_shares)
                {
                    match frost::keygen::verify_encrypted_secret_share(
                        sharing_state,
                        event.participant,
                        encrypted_shares,
                    ) {
                        Ok(share) => {
                            shares.insert(event.participant, share);
                        }
                        Err(err) => {
                            tracing::warn!(
                                %err,
                                participant = %event.participant,
                                "invalid key gen encrypted secret share"
                            );

                            // The complaint actions have a whole other key gen
                            // timeout to arrive onchain.
                            let expires_at = deadline
                                .map(|_| block.saturating_add(self.config.key_gen_timeout.get()));

                            commands.push(Command::Action(Action::KeyGenComplain {
                                group_id: group.id(),
                                accused: event.participant,
                                expires_at,
                            }));
                        }
                    }
                }

                if public_keys.len() as u16 != count {
                    // We are still missing shares from some participants, so
                    // stay in the same collecting state.
                    return (
                        State {
                            rollover: RolloverState::CollectingShares {
                                next_epoch,
                                group,
                                participation,
                                public_keys,
                                shares,
                                complaints,
                                deadline,
                            },
                            ..state
                        },
                        commands,
                    );
                }

                // Every participant has submitted a share, so a fresh round
                // starts: push the deadlines forward from the current block.
                let deadlines = deadline.map(|_| ConfirmationDeadlines {
                    complain: block.saturating_add(self.config.key_gen_timeout.get()),
                    response: block
                        .saturating_add(self.config.key_gen_timeout.get().saturating_mul(2)),
                    confirm: block
                        .saturating_add(self.config.key_gen_timeout.get().saturating_mul(3)),
                });

                // Finalize our key share only if every share we received was
                // valid; otherwise wait for the complaint flow to resolve.
                let status = match &participation {
                    KeyGenParticipation::Participating(sharing_state)
                        if shares.len() as u16 == count =>
                    {
                        match self.confirm_key_gen(
                            next_epoch,
                            &group,
                            sharing_state.clone(),
                            shares,
                            deadlines.as_ref(),
                        ) {
                            Ok((status, confirm_commands)) => {
                                commands.extend(confirm_commands);
                                status
                            }
                            Err(err) => {
                                // Finalization failures are unexpected, since
                                // all secret shares were already verified.
                                return fail_rollover!(state, next_epoch, err);
                            }
                        }
                    }
                    KeyGenParticipation::Participating(_) => KeyGenConfirmation::Collecting(shares),
                    KeyGenParticipation::Observing(_) => KeyGenConfirmation::Observing,
                };

                (
                    State {
                        rollover: RolloverState::CollectingConfirmations {
                            next_epoch,
                            group,
                            participation,
                            status,
                            confirmations: BTreeSet::new(),
                            complaints,
                            deadlines,
                        },
                        ..state
                    },
                    commands,
                )
            }
            _ => (state, Vec::new()),
        }
    }

    /// Registers a peer's confirmation of a completed key generation.
    pub(super) fn handle_key_gen_confirmed(
        &self,
        mut state: State,
        block: u64,
        event: &Coordinator::KeyGenConfirmed,
    ) -> (State, Commands<State, Self>) {
        match state.rollover {
            RolloverState::CollectingConfirmations {
                next_epoch,
                group,
                participation,
                status,
                mut confirmations,
                complaints,
                deadlines,
            } if group.id() == event.gid => {
                let (count, _) = group.size();

                confirmations.insert(event.participant);
                if confirmations.len() as u16 != count {
                    // We are still missing confirmations from some
                    // participants, or this is a non-genesis confirmation
                    // whose rollover-packet branch isn't wired in yet.
                    return (
                        State {
                            rollover: RolloverState::CollectingConfirmations {
                                next_epoch,
                                group,
                                participation,
                                status,
                                confirmations,
                                complaints,
                                deadlines,
                            },
                            ..state
                        },
                        Vec::new(),
                    );
                }

                match next_epoch {
                    // On genesis: retain the active key, start preprocessing,
                    // and immediately begin key generation for the next epoch.
                    EpochId::Genesis => {
                        let next_epoch = epoch::next_number(block, self.config.blocks_per_epoch);
                        let (state, finalize_commands) = self.finalize_key_gen(
                            State {
                                rollover: RolloverState::EpochSkipped { next_epoch },
                                ..state
                            },
                            EpochId::Genesis,
                            group,
                            status.key_share(),
                        );

                        // Kick off the next keygen ceremony right away.
                        let Some(participants) = group::participants_set(
                            &self.config.participants,
                            group::Epoch::Number {
                                consensus: self.config.consensus,
                                number: next_epoch,
                                excluded: BTreeSet::new(),
                            },
                        ) else {
                            return (state, finalize_commands);
                        };

                        let deadline =
                            Some(block.saturating_add(self.config.key_gen_timeout.get()));
                        let (state, keygen_commands) = self.start_key_gen(
                            state,
                            EpochId::Number { number: next_epoch },
                            &participants,
                            deadline,
                        );

                        (state, [finalize_commands, keygen_commands].concat())
                    }
                    EpochId::Number {
                        number: proposed_epoch,
                    } => {
                        let group_id = group.id();
                        let key_share = status.key_share();

                        // Compute the rollover package that needs to be
                        // attested for the epoch to get staged.
                        let active_epoch = state.active_epoch;
                        let group_key = participation.group_commitments().group_key();
                        let rollover_block = proposed_epoch
                            .get()
                            .saturating_mul(self.config.blocks_per_epoch.get());
                        let message = self.consensus.epoch_rollover_hash(
                            active_epoch,
                            proposed_epoch,
                            rollover_block,
                            &group_key,
                        );
                        // If we are participating in the active epoch, then
                        // also register the rollover packet for signing.
                        if let Some(participating_epoch) = state.epochs.get(&active_epoch) {
                            state.signing.insert(
                                message,
                                SigningState::WaitingForRequest {
                                    key_share: participating_epoch.key_share.clone(),
                                    group_id: participating_epoch.group.id(),
                                    responsible: Some(event.participant),
                                    packet: Packet::EpochRollover {
                                        active_epoch,
                                        proposed_epoch,
                                        rollover_block,
                                        group_id,
                                        group_key,
                                    },
                                    signers: participating_epoch.group.participants().clone(),
                                    deadline: block
                                        .saturating_add(self.config.signing_timeout.get()),
                                },
                            );
                        };

                        (
                            State {
                                rollover: RolloverState::SigningRollover {
                                    next_epoch: proposed_epoch,
                                    group,
                                    key_share,
                                    message,
                                },
                                ..state
                            },
                            Vec::new(),
                        )
                    }
                }
            }
            _ => (state, Vec::new()),
        }
    }

    /// Finalizes a staged epoch once its rollover attestation lands onchain:
    /// clears the rollover signing session and moves
    /// [`RolloverState::SigningRollover`] to [`RolloverState::EpochStaged`],
    /// via [`Self::finalize_key_gen`] recording the new epoch and group in
    /// `state.epochs` (if this validator is participating) and preprocessing
    /// it by sampling and registering its nonce tree. Ports
    /// `consensus/epochStaged.ts`.
    ///
    /// `active_epoch` itself is not rolled forward here; that only happens
    /// later, once the block clock reaches the rollover block (see
    /// [`Self::handle_rollover_new_block`]).
    pub(super) fn handle_epoch_staged(
        &self,
        state: State,
        event: &Consensus::EpochStaged,
    ) -> (State, Commands<State, Self>) {
        match state.rollover {
            RolloverState::SigningRollover {
                next_epoch,
                group,
                key_share,
                message,
            } if group.id() == event.groupId => {
                let epoch = EpochId::Number { number: next_epoch };
                tracing::info!(
                    %next_epoch,
                    group_id = %event.groupId,
                    signature_id = %event.signatureId,
                    "epoch staged"
                );
                let (state, keygen_commands) = self.finalize_key_gen(
                    State {
                        rollover: RolloverState::EpochStaged { next_epoch },
                        ..state
                    },
                    epoch,
                    group,
                    key_share,
                );
                let (state, attested_commands) =
                    self.handle_sign_attested(state, event.signatureId, message);

                (state, [keygen_commands, attested_commands].concat())
            }
            RolloverState::WaitingForGenesis => {
                // We should have been waiting for genesis to start. In this,
                // optimistically jump to skipping to the proposed epoch. This
                // has the added benefit that if a validator joins partway
                // through consensus, the will eventually recover and not get
                // stuck forever waiting for genesis.
                tracing::warn!(
                    proposed_epoch = event.proposedEpoch,
                    group_id = %event.groupId,
                    "observed staged epoch before genesis; recovering from staged epoch"
                );
                (
                    State {
                        rollover: NonZeroU64::new(event.proposedEpoch)
                            .map(|next_epoch| RolloverState::EpochSkipped { next_epoch })
                            // The contract should disallow proposing genesis
                            // epochs, something is very wrong...
                            .unwrap_or(RolloverState::Halted),
                        ..state
                    },
                    Vec::new(),
                )
            }
            _ => {
                tracing::warn!(
                    epoch = %event.proposedEpoch,
                    "an unexpected epoch was staged; key generation may time out"
                );

                // We saw an epoch get staged when we were not expecting one.
                // Either way, we should recover and clean up through timeouts.
                (state, Vec::new())
            }
        }
    }

    /// Registers a complaint raised against a participant. Once enough
    /// complaints have accrued against a single participant to reach the
    /// group's signing threshold, key generation restarts excluding them;
    /// otherwise, if this validator is the one accused, it reveals its own
    /// secret share for the plaintiff via [`Action::KeyGenComplaintResponse`].
    pub(super) fn handle_key_gen_complained(
        &self,
        mut state: State,
        block: u64,
        event: &Coordinator::KeyGenComplained,
    ) -> (State, Commands<State, Self>) {
        let (next_epoch, group, participation, complaints, restart_deadline, response_expires_at) =
            match &mut state.rollover {
                RolloverState::CollectingShares {
                    next_epoch,
                    group,
                    participation,
                    complaints,
                    deadline,
                    ..
                } if group.id() == event.gid => {
                    let restart_deadline =
                        deadline.map(|_| block.saturating_add(self.config.key_gen_timeout.get()));
                    // We get at least another `key_gen_timeout` to get the
                    // complaint response onchain, which ends up being the same
                    // value as the restart deadline (by coincidence).
                    let response_expires_at = restart_deadline;
                    (
                        *next_epoch,
                        &*group,
                        &*participation,
                        complaints,
                        restart_deadline,
                        response_expires_at,
                    )
                }
                RolloverState::CollectingConfirmations {
                    next_epoch,
                    group,
                    participation,
                    complaints,
                    deadlines,
                    ..
                } if group.id() == event.gid
                    && deadlines
                        .as_ref()
                        .is_none_or(|deadlines| block <= deadlines.complain) =>
                {
                    let restart_deadline = deadlines
                        .as_ref()
                        .map(|_| block.saturating_add(self.config.key_gen_timeout.get()));
                    let response_expires_at =
                        deadlines.as_ref().map(|deadlines| deadlines.response);
                    (
                        *next_epoch,
                        &*group,
                        &*participation,
                        complaints,
                        restart_deadline,
                        response_expires_at,
                    )
                }
                _ => return (state, Vec::new()),
            };

        let complaint = complaints.entry(event.accused).or_default();
        complaint.total += 1;
        complaint.unresponded += 1;

        // If we ever get threshold complaints, the keygen is done. This is
        // because it would reveal sufficient public information to compute
        // secret key shares from one or more participants.
        let (_, threshold) = group.size();
        if complaint.total >= threshold {
            tracing::warn!(
                accused = %event.accused,
                "restarting key generation after too many complaints"
            );

            let excluded = group.also_exclude(iter::once(event.accused));

            return self.restart_key_gen_excluding(state, next_epoch, excluded, restart_deadline);
        }

        let mut commands = Vec::new();
        if let KeyGenParticipation::Participating(sharing_state) = participation
            && event.accused == self.account
        {
            match frost::keygen::reveal_secret_share(sharing_state, event.plaintiff) {
                Ok(secret_share) => {
                    commands.push(Command::Action(Action::KeyGenComplaintResponse {
                        group_id: group.id(),
                        plaintiff: event.plaintiff,
                        secret_share,
                        expires_at: response_expires_at,
                    }));
                }
                Err(err) => {
                    tracing::warn!(
                        %err,
                        plaintiff = %event.plaintiff,
                        "failed to reveal secret share for complaint response"
                    );
                }
            }
        }

        (state, commands)
    }

    /// Registers a revealed secret share published in response to a complaint.
    /// If this validator is the complaint's plaintiff, the revealed share is
    /// registered as its own - finalizing and confirming the key share if that
    /// was the last one missing; otherwise, the revealed share is simply
    /// verified against the accused's public commitment. An invalid revealed
    /// share restarts key generation excluding the accused.
    pub(super) fn handle_key_gen_complaint_responded(
        &self,
        mut state: State,
        block: u64,
        event: &Coordinator::KeyGenComplaintResponded,
    ) -> (State, Commands<State, Self>) {
        let (next_epoch, group, participation, shares, complaints, deadline) =
            match &mut state.rollover {
                RolloverState::CollectingShares {
                    next_epoch,
                    group,
                    participation,
                    shares,
                    complaints,
                    deadline,
                    ..
                } if group.id() == event.gid => (
                    *next_epoch,
                    &*group,
                    &*participation,
                    Some(shares),
                    complaints,
                    *deadline,
                ),
                RolloverState::CollectingConfirmations {
                    next_epoch,
                    group,
                    participation,
                    status,
                    complaints,
                    deadlines,
                    ..
                } if group.id() == event.gid
                    && deadlines
                        .as_ref()
                        .is_none_or(|deadlines| block <= deadlines.response) =>
                {
                    let shares = if let KeyGenConfirmation::Collecting(shares) = status {
                        Some(shares)
                    } else {
                        None
                    };
                    let deadline = deadlines.as_ref().map(|deadlines| deadlines.response);

                    (
                        *next_epoch,
                        &*group,
                        &*participation,
                        shares,
                        complaints,
                        deadline,
                    )
                }
                _ => return (state, Vec::new()),
            };

        let Some(complaint) = complaints
            .get_mut(&event.accused)
            .filter(|complaint| complaint.unresponded > 0)
        else {
            return (state, Vec::new());
        };

        match frost::keygen::verify_revealed_secret_share(
            participation.group_commitments(),
            event.plaintiff,
            event.accused,
            event.secretShare,
        ) {
            Ok(share) => {
                if let Some(shares) = shares
                    && event.plaintiff == self.account
                {
                    shares.insert(event.accused, share);
                }
                complaint.unresponded -= 1;
            }
            Err(err) => {
                tracing::warn!(
                    %err,
                    accused = %event.accused,
                    "invalid secret share revealed in response to a complaint"
                );

                let excluded = group.also_exclude(iter::once(event.accused));
                let restart_deadline =
                    deadline.map(|_| block.saturating_add(self.config.key_gen_timeout.get()));

                return self.restart_key_gen_excluding(
                    state,
                    next_epoch,
                    excluded,
                    restart_deadline,
                );
            }
        }

        // In case we are in the confirmation phase, collecting shares, and
        // got the final share, we have to finalize the keygen process and emit
        // a keygen confirmation action.
        let mut commands = Vec::new();
        if let RolloverState::CollectingConfirmations {
            group,
            participation,
            status,
            deadlines,
            ..
        } = &mut state.rollover
        {
            let (count, _) = group.size();
            if let (
                KeyGenParticipation::Participating(sharing_state),
                KeyGenConfirmation::Collecting(shares),
            ) = (participation, &mut *status)
                && shares.len() as u16 == count
            {
                let sharing_state = sharing_state.clone();
                *status = match self.confirm_key_gen(
                    next_epoch,
                    group,
                    sharing_state,
                    mem::take(shares),
                    deadlines.as_ref(),
                ) {
                    Ok((new_status, confirm_commands)) => {
                        commands.extend(confirm_commands);
                        new_status
                    }
                    Err(err) => {
                        // Finalization failures are unexpected, as all secret
                        // shares were already verified.
                        return fail_rollover!(state, next_epoch, err);
                    }
                };
            }
        }

        (state, commands)
    }

    /// Drives the epoch-rollover machine forward on the block clock: once
    /// the block reaches the rollover state's target epoch, stages the
    /// epoch (rolling `active_epoch` forward) if it was ready, then triggers
    /// a fresh key generation for whichever epoch is actually due now -
    /// abandoning whatever attempt was in flight for a now-stale target.
    ///
    /// Genesis groups do not observe the rollover clock.
    pub(super) fn handle_rollover_new_block(
        &self,
        mut state: State,
        block: u64,
    ) -> (State, Commands<State, Self>) {
        let Some(target_epoch_number) =
            state.rollover.next_epoch().and_then(|epoch| epoch.number())
        else {
            return (state, Vec::new());
        };

        let next_epoch_number = epoch::next_number(block, self.config.blocks_per_epoch);
        if target_epoch_number >= next_epoch_number {
            // Not due yet.
            return (state, Vec::new());
        }

        // In case an epoch was staged, make it the new active epoch.
        if let RolloverState::EpochStaged { .. } = state.rollover {
            tracing::info!(
                active_epoch = %target_epoch_number,
                block,
                "epoch rolled over"
            );
            state.active_epoch = EpochId::Number {
                number: target_epoch_number,
            };
        } else {
            tracing::warn!(
                target_epoch = %target_epoch_number,
                next_epoch = %next_epoch_number,
                block,
                "epoch key generation was not staged in time; abandoning stale attempt"
            );
        }

        // Start a new keygen ceremony for the new next block, including
        // everyone again.
        let deadline = Some(block.saturating_add(self.config.key_gen_timeout.get()));
        let next_epoch = EpochId::Number {
            number: next_epoch_number,
        };
        let participants = group::participants_set(
            &self.config.participants,
            group::Epoch::Number {
                consensus: self.config.consensus,
                number: next_epoch_number,
                excluded: BTreeSet::new(),
            },
        );

        match participants {
            Some(participants) => self.start_key_gen(state, next_epoch, &participants, deadline),
            None => {
                tracing::warn!(
                    ?next_epoch,
                    "could not establish a fresh participant set for a new epoch; skipping"
                );
                (
                    State {
                        rollover: RolloverState::EpochSkipped {
                            next_epoch: next_epoch_number,
                        },
                        ..state
                    },
                    Vec::new(),
                )
            }
        }
    }

    /// Retires participants whose round of the current key generation has
    /// stalled past its deadline, restarting excluding them.
    ///
    /// Every "collecting" round is checked against its own deadline:
    /// commitments/shares against participants who haven't yet submitted one,
    /// confirmations first (once the response deadline passes) against
    /// participants with an unanswered complaint, or (once the confirm
    /// deadline passes) against everyone who hasn't confirmed. Genesis's key
    /// generation has no deadline and is therefore never retried this way.
    pub(super) fn handle_key_gen_timeouts(
        &self,
        state: State,
        block: u64,
    ) -> (State, Commands<State, Self>) {
        // A commitment round every participant committed to, but whose own key
        // generation setup never resumed, cannot be restarted: no participant
        // is missing to exclude, so the restart would compute an identical
        // group and publish a second commitment over the one already onchain.
        // Skip the epoch instead. Other validators will just see us as not
        // participating in the secret sharing phase and timeout there.
        let stuck = match &state.rollover {
            RolloverState::CollectingCommitments {
                next_epoch,
                group,
                secrets: KeyGenCommitment::Participating { secrets: None, .. },
                commitments,
                deadline: Some(deadline),
            } if block >= *deadline && commitments.len() as u16 == group.size().0 => {
                tracing::error!(
                    ?next_epoch,
                    group_id = %group.id(),
                    block,
                    deadline,
                    "key generation setup did not complete before its round closed"
                );
                Some(*next_epoch)
            }
            _ => None,
        };
        if let Some(next_epoch) = stuck {
            return fail_rollover!(state, next_epoch, "key generation setup did not complete");
        }

        let Some((next_epoch, excluded)) = (match &state.rollover {
            RolloverState::CollectingCommitments {
                next_epoch,
                group,
                commitments,
                deadline: Some(deadline),
                ..
            } if block >= *deadline => {
                // There are participants did did not commit, restart keygen
                // without them.
                tracing::warn!(
                    ?next_epoch,
                    group_id = %group.id(),
                    block,
                    deadline,
                    committed = ?commitments.keys().copied().collect::<BTreeSet<_>>(),
                    "key generation commitment collection timed out"
                );
                let excluded = group.exclude_all_others(commitments.keys());
                Some((*next_epoch, excluded))
            }
            RolloverState::CollectingShares {
                next_epoch,
                group,
                public_keys,
                deadline: Some(deadline),
                ..
            } if block >= *deadline => {
                // There are participants that did not submit secret shares
                // onchain. Note that we use the `public_keys` map to determine
                // which participants are missing and not `shares`: this is
                // because `shares` contains verified shares, which may be
                // added later through the complaint flow.
                tracing::warn!(
                    ?next_epoch,
                    group_id = %group.id(),
                    block,
                    deadline,
                    shared = ?public_keys.keys().copied().collect::<BTreeSet<_>>(),
                    "key generation share collection timed out"
                );
                let excluded = group.exclude_all_others(public_keys.keys());
                Some((*next_epoch, excluded))
            }
            RolloverState::CollectingConfirmations {
                next_epoch,
                group,
                complaints,
                confirmations,
                deadlines: Some(deadlines),
                ..
            } => {
                let unresponded = complaints
                    .iter()
                    .filter(|(_, complaint)| complaint.unresponded > 0)
                    .map(|(address, _)| *address)
                    .collect::<BTreeSet<_>>();
                let excluded = if block >= deadlines.response && !unresponded.is_empty() {
                    // There are unresponded complaints past the response deadline,
                    // exclude all participants that failed to respond.
                    Some(group.also_exclude(unresponded))
                } else if block >= deadlines.confirm {
                    // There are missing confirmations past the confirmation
                    // deadline, exclude participants that did not confirm.
                    Some(group.exclude_all_others(confirmations))
                } else {
                    None
                };
                excluded.map(|excluded| (*next_epoch, excluded))
            }
            _ => None,
        }) else {
            // No timeout occurred, continue on our merry way...
            return (state, Vec::new());
        };

        tracing::warn!(
            ?next_epoch,
            ?excluded,
            "key generation timed out, restarting excluding stalled participants",
        );
        let deadline = Some(block.saturating_add(self.config.key_gen_timeout.get()));
        self.restart_key_gen_excluding(state, next_epoch, excluded, deadline)
    }

    /// Starts a key generation ceremony for `next_epoch` with `participants`,
    /// entering [`RolloverState::WaitingForSetup`] if this validator is part of
    /// the group, or heading straight to
    /// [`RolloverState::CollectingCommitments`] as an observer otherwise.
    fn start_key_gen(
        &self,
        state: State,
        next_epoch: EpochId,
        participants: &ParticipantSet,
        deadline: Option<u64>,
    ) -> (State, Commands<State, Self>) {
        // Only participate in the group generation if you are part of the
        // participant set; otherwise go straight to collecting the other
        // participants' commitments.
        if let Some((group, poap)) = participants.participate_as(self.account) {
            let group_id = group.id();
            let (count, threshold) = group.size();
            tracing::info!(
                ?next_epoch,
                %group_id,
                count,
                threshold,
                "starting key generation"
            );
            (
                State {
                    rollover: RolloverState::CollectingCommitments {
                        next_epoch,
                        group,
                        secrets: KeyGenCommitment::Participating {
                            poap,
                            secrets: None,
                        },
                        commitments: BTreeMap::new(),
                        deadline,
                    },
                    ..state
                },
                vec![Command::Effect(Effect::KeyGenSetup {
                    group_id,
                    count,
                    threshold,
                })],
            )
        } else {
            let group = participants.group();
            let group_id = group.id();
            let (count, threshold) = group.size();
            tracing::info!(
                ?next_epoch,
                %group_id,
                count,
                threshold,
                "observing key generation"
            );
            (
                State {
                    rollover: RolloverState::CollectingCommitments {
                        next_epoch,
                        group,
                        secrets: KeyGenCommitment::Observing,
                        commitments: BTreeMap::new(),
                        deadline,
                    },
                    ..state
                },
                Vec::new(),
            )
        }
    }

    /// Restarts key generation for `next_epoch` excluding `excluded`, or
    /// halts/skips the epoch (via [`rollover_failure`], logging `reason`) if
    /// too few participants would remain -- or if `next_epoch` is the genesis
    /// epoch, which cannot be restarted since its group ID is externally
    /// authorized onchain and any restart would produce a different,
    /// unauthorized one.
    fn restart_key_gen_excluding(
        &self,
        state: State,
        next_epoch: EpochId,
        excluded: BTreeSet<Address>,
        deadline: Option<u64>,
    ) -> (State, Commands<State, Self>) {
        let participants = if let EpochId::Number { number } = next_epoch {
            group::participants_set(
                &self.config.participants,
                group::Epoch::Number {
                    consensus: self.config.consensus,
                    number,
                    excluded,
                },
            )
        } else {
            // In case we need to restart keygen during genesis - halt! The
            // The genesis keygen is special in that it cannot be restart
            // since the group ID has special authorization, and any restart
            // would issue a new and different group ID
            None
        };

        match participants {
            Some(participants) => self.start_key_gen(state, next_epoch, &participants, deadline),
            None => (
                State {
                    rollover: rollover_failure(
                        next_epoch,
                        "could not form new participant set to restart keygen",
                    ),
                    ..state
                },
                Vec::new(),
            ),
        }
    }

    /// Finalizes a commitment round every participant has committed to,
    /// producing the [`RolloverState::CollectingShares`] state that follows
    /// it: kicking off this validator's secret share if it is participating,
    /// or just computing the group commitments it needs as an observer.
    ///
    /// `secrets` are this validator's key generation secrets, or `None` if it
    /// is only participating as an observer. `deadline` is the already
    /// computed deadline for the secret-share round, since the two ways out of
    /// the commitment round do not agree on a block to derive it from: only
    /// the last commitment arrives with one.
    fn finalize_key_gen_commitments(
        &self,
        next_epoch: EpochId,
        group: Group,
        secrets: Option<Box<Secrets>>,
        commitments: BTreeMap<Address, VerifiedCommitment>,
        deadline: Option<u64>,
    ) -> (RolloverState, Commands<State, Self>) {
        // Compute the group participation state for the validator, depending
        // on whether or not it is observing.
        let (participation, commands) = match if let Some(secrets) = secrets {
            // We are participating, so compute the secret shares to publish
            // onchain and the sharing state.
            frost::keygen::generate_secret_shares(*secrets, commitments).map(
                |(sharing_state, share)| {
                    let participation = KeyGenParticipation::Participating(sharing_state);
                    let commands = vec![Command::Action(Action::KeyGenSecretShare {
                        group_id: group.id(),
                        share,
                        expires_at: deadline,
                    })];
                    (participation, commands)
                },
            )
        } else {
            // We are observing, just compute the group commitments that are
            // required to verify secret share public key shares and complain
            // responses.
            frost::keygen::group_commitments(commitments).map(|group_commitments| {
                let participation = KeyGenParticipation::Observing(group_commitments);
                (participation, Vec::new())
            })
        } {
            Ok(result) => result,
            Err(err) => {
                // There was an issue with the verified commitments, which is
                // an unexpected an unrecoverable error.
                return (rollover_failure(next_epoch, err), Vec::new());
            }
        };

        (
            RolloverState::CollectingShares {
                next_epoch,
                group,
                participation,
                public_keys: BTreeMap::new(),
                shares: BTreeMap::new(),
                complaints: BTreeMap::new(),
                deadline,
            },
            commands,
        )
    }

    /// Finalizes keygen and triggers nonces preprocessing. Any remaining DKG
    /// secrets are pruned by the next group reconciliation, since the group
    /// now has a key share (or none, if not participating).
    fn finalize_key_gen(
        &self,
        mut state: State,
        epoch: EpochId,
        group: Group,
        key_share: Option<Arc<KeyShare>>,
    ) -> (State, Commands<State, Self>) {
        let group_id = group.id();

        // If we are participating in the new group (in other words, we were
        // part of the DKG ceremony and key share), register the epoch in our
        // participating epochs map and generate a nonces chunk.
        let commands = if let Some(key_share) = key_share {
            let mut nonces = NonceState::default();
            let chunk = nonces.reserve_chunk();
            debug_assert_eq!(chunk, Some(0));

            state.epochs.insert(
                epoch,
                Epoch {
                    group,
                    key_share,
                    nonces,
                },
            );
            vec![Command::Effect(Effect::NonceTree { group_id })]
        } else {
            Vec::new()
        };

        (state, commands)
    }

    /// Encodes commands for confirming a newly established secret key share,
    /// returning the updated confirmation status.
    fn confirm_key_gen(
        &self,
        epoch: EpochId,
        group: &Group,
        sharing_state: SharingState,
        shares: BTreeMap<Address, VerifiedShare>,
        deadlines: Option<&ConfirmationDeadlines>,
    ) -> Result<(KeyGenConfirmation, Commands<State, Self>), frost::error::Error> {
        let key_share = frost::keygen::finalize(sharing_state, shares)?;
        let key_share = Arc::new(key_share);

        Ok((
            KeyGenConfirmation::Confirmed(key_share.clone()),
            vec![
                Command::Action(Action::KeyGenConfirm {
                    group_id: group.id(),
                    callback: self.key_gen_confirmation_callback(epoch),
                    expires_at: deadlines.map(|deadlines| deadlines.confirm),
                }),
                // Preemptively start nonce generation before any group
                // reconciliation effect. This allows us to compute a nonce tree
                // early to ensure we have one available for when we need it.
                Command::Effect(Effect::StartNonceGeneration {
                    group_id: group.id(),
                    key_share,
                }),
            ],
        ))
    }

    /// Builds the callback that proposes a regular epoch as soon as the final
    /// participant confirms its generated key. Genesis is externally
    /// bootstrapped and does not need a callback.
    fn key_gen_confirmation_callback(
        &self,
        next_epoch: EpochId,
    ) -> Option<crate::bindings::Callback> {
        let EpochId::Number { number } = next_epoch else {
            return None;
        };

        Some(crate::bindings::Callback {
            target: self.config.consensus,
            context: (
                number.get(),
                number
                    .get()
                    .saturating_mul(self.config.blocks_per_epoch.get()),
            )
                .abi_encode()
                .into(),
        })
    }
}

impl RolloverState {
    /// The epoch this rollover state is working toward becoming active, or
    /// `None` for the terminal [`Self::Halted`] state.
    fn next_epoch(&self) -> Option<EpochId> {
        match self {
            RolloverState::WaitingForGenesis => Some(EpochId::Genesis),
            RolloverState::EpochSkipped { next_epoch }
            | RolloverState::SigningRollover { next_epoch, .. }
            | RolloverState::EpochStaged { next_epoch, .. } => Some(EpochId::Number {
                number: *next_epoch,
            }),
            RolloverState::CollectingCommitments { next_epoch, .. }
            | RolloverState::CollectingShares { next_epoch, .. }
            | RolloverState::CollectingConfirmations { next_epoch, .. } => Some(*next_epoch),
            RolloverState::Halted => None,
        }
    }
}

impl KeyGenParticipation {
    /// Gets the group commitments for regardless of participation.
    fn group_commitments(&self) -> &GroupCommitments {
        match self {
            KeyGenParticipation::Participating(sharing_state) => sharing_state.group_commitments(),
            KeyGenParticipation::Observing(group_commitments) => group_commitments,
        }
    }
}

impl KeyGenConfirmation {
    /// Returns the finalized key share, if this validator's confirmation
    /// completed successfully.
    fn key_share(&self) -> Option<Arc<KeyShare>> {
        match self {
            KeyGenConfirmation::Confirmed(key_share) => Some(key_share.clone()),
            _ => None,
        }
    }
}

/// Handle a FROST error and return the next rollover state.
fn rollover_failure(next_epoch: EpochId, err: impl Display) -> RolloverState {
    if let EpochId::Number { number: next_epoch } = next_epoch {
        tracing::warn!(
            %err,
            ?next_epoch,
            "failed to advance key generation, skipping to next epoch"
        );
        RolloverState::EpochSkipped { next_epoch }
    } else {
        tracing::error!(
            %err,
            "failed to advance genesis key generation, permanently halted"
        );
        RolloverState::Halted
    }
}

// This is a macro instead of a function because keygen handlers partially move
// `state.rollover` while matching its fields. The remaining `state` therefore
// cannot be passed whole to a function, but a macro can reconstruct it at the
// call site.
macro_rules! fail_rollover {
    ($state:ident, $next_epoch:expr, $err:expr) => {{
        let (next_epoch, err) = ($next_epoch, $err);
        (
            State {
                rollover: rollover_failure(next_epoch, err),
                ..($state)
            },
            Vec::new(),
        )
    }};
}
use fail_rollover;
