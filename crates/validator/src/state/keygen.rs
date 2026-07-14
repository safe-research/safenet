use super::{
    ConfirmationDeadlines, Epoch, KeyGenConfirmation, KeyGenParticipation, Packet, Prune,
    RolloverState, SigningState, State, Transition,
};
use crate::{
    bindings::Coordinator,
    consensus::{
        epoch::{self, EpochId},
        group::{self, ParticipantSet},
    },
    frost::{
        self,
        keygen::{GroupCommitments, Secrets},
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
    /// effect has produced it, moving the group into commitment collection.
    pub(super) fn handle_key_gen_setup(
        &self,
        state: State,
        group_id: B256,
        secrets: Box<Secrets>,
    ) -> (State, Commands<State, Self>) {
        match state.rollover {
            RolloverState::WaitingForSetup {
                next_epoch,
                group,
                poap,
                deadline,
            } if group_id == group.id() => {
                let (participants, count, threshold, context) = group.parameters();
                let commitment = secrets.commitment();
                (
                    State {
                        rollover: RolloverState::CollectingCommitments {
                            next_epoch,
                            group,
                            secrets: Some(secrets),
                            commitments: BTreeMap::new(),
                            deadline,
                        },
                        ..state
                    },
                    vec![Command::Action(Action::KeyGenAndCommit {
                        participants,
                        count,
                        threshold,
                        context,
                        poap,
                        commitment,
                        expires_at: deadline,
                    })],
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

                // Every participant has committed, so a fresh round starts:
                // push the deadline forward from the current block. Genesis
                // is not subject to a rollover deadline, so `None` stays
                // `None`.
                let deadline =
                    deadline.map(|_| block.saturating_add(self.config.key_gen_timeout.get()));

                // Compute the group participation state for the validator,
                // depending on whether or not it is observing.
                let (participation, commands) = match if let Some(secrets) = secrets {
                    // We are participating, so compute the secret shares to
                    // publish onchain and the sharing state.
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
                    // We are observing, just compute the group commitments that
                    // are required to verify secret share public key shares and
                    // complain responses.
                    frost::keygen::group_commitments(commitments).map(|group_commitments| {
                        let participation = KeyGenParticipation::Observing(group_commitments);
                        (participation, Vec::new())
                    })
                } {
                    Ok(result) => result,
                    Err(err) => {
                        // There was an issue with the verified commitments,
                        // which is an unexpected an unrecoverable error.
                        return fail_rollover!(state, block, next_epoch, group.id(), err);
                    }
                };

                (
                    State {
                        rollover: RolloverState::CollectingShares {
                            next_epoch,
                            group,
                            participation: Box::new(participation),
                            public_keys: BTreeMap::new(),
                            shares: BTreeMap::new(),
                            complaints: BTreeMap::new(),
                            deadline,
                        },
                        ..state
                    },
                    commands,
                )
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
                    (&*participation, encrypted_shares)
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
                let status = match &*participation {
                    KeyGenParticipation::Participating(sharing_state)
                        if shares.len() as u16 == count =>
                    {
                        match frost::keygen::finalize(sharing_state.clone(), shares) {
                            Ok(key_share) => {
                                commands.push(Command::Action(Action::KeyGenConfirm {
                                    group_id: group.id(),
                                    callback: self.key_gen_confirmation_callback(next_epoch),
                                    expires_at: deadlines
                                        .as_ref()
                                        .map(|deadlines| deadlines.confirm),
                                }));
                                KeyGenConfirmation::Confirmed(Arc::new(key_share))
                            }
                            Err(err) => {
                                // Finalization failures are unexpected, since
                                // all secret shares were already verified.
                                return fail_rollover!(state, block, next_epoch, group.id(), err);
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
                        let group_id = group.id();
                        let mut commands = if let KeyGenConfirmation::Confirmed(key_share) = status
                        {
                            state.epochs.insert(
                                next_epoch,
                                Epoch {
                                    group,
                                    key_share: key_share.clone(),
                                },
                            );
                            vec![Command::Effect(Effect::NonceTree {
                                group_id,
                                key_share,
                            })]
                        } else {
                            Vec::new()
                        };

                        let next_epoch = epoch::next_number(block, self.config.blocks_per_epoch);
                        let state = State {
                            rollover: RolloverState::EpochSkipped { next_epoch },
                            ..state
                        }
                        .and_prune(block, Prune::KeyGenSecrets { group_id });

                        let Some(participants) = group::participants_set(
                            &self.config.participants,
                            group::Epoch::Number {
                                consensus: self.config.consensus,
                                number: next_epoch,
                                excluded: BTreeSet::new(),
                            },
                        ) else {
                            return (state, commands);
                        };

                        let deadline =
                            Some(block.saturating_add(self.config.key_gen_timeout.get()));
                        let (state, mut keygen_commands) = self.start_key_gen(
                            state,
                            EpochId::Number { number: next_epoch },
                            &participants,
                            deadline,
                        );
                        commands.append(&mut keygen_commands);

                        (state, commands)
                    }
                    EpochId::Number {
                        number: proposed_epoch,
                    } => {
                        let group_id = group.id();

                        // Regardless of whether or not we are part of the
                        // active epoch and will participate in the signing
                        // ceremony to generate the rollover attestation,
                        // register the newly created epoch and group.
                        if let KeyGenConfirmation::Confirmed(key_share) = status {
                            state.epochs.insert(next_epoch, Epoch { group, key_share });
                        }

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
                                    packet: Packet::EpochRollover {
                                        active_epoch,
                                        proposed_epoch,
                                        rollover_block,
                                        group_id,
                                        group_key,
                                    },
                                    signers: participating_epoch.group.participants().clone(),
                                    deadline: Some(
                                        block.saturating_add(self.config.signing_timeout.get()),
                                    ),
                                },
                            );
                        };

                        (
                            State {
                                rollover: RolloverState::SigningRollover {
                                    next_epoch: proposed_epoch,
                                    group_id,
                                    message,
                                },
                                ..state
                            }
                            .and_prune(block, Prune::KeyGenSecrets { group_id }),
                            Vec::new(),
                        )
                    }
                }
            }
            _ => (state, Vec::new()),
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

            let group_id = group.id();
            let excluded = group.also_exclude(iter::once(event.accused));

            return self.restart_key_gen_excluding(
                state.and_prune(block, Prune::KeyGenSecrets { group_id }),
                next_epoch,
                excluded,
                restart_deadline,
            );
        }

        let mut commands = Vec::new();
        if let KeyGenParticipation::Participating(sharing_state) = &**participation
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

                let group_id = group.id();
                let excluded = group.also_exclude(iter::once(event.accused));
                let restart_deadline =
                    deadline.map(|_| block.saturating_add(self.config.key_gen_timeout.get()));

                return self.restart_key_gen_excluding(
                    state.and_prune(block, Prune::KeyGenSecrets { group_id }),
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
            ) = (&**participation, &mut *status)
                && shares.len() as u16 == count
            {
                let sharing_state = sharing_state.clone();
                match frost::keygen::finalize(sharing_state, mem::take(shares)) {
                    Ok(key_share) => {
                        commands.push(Command::Action(Action::KeyGenConfirm {
                            group_id: group.id(),
                            callback: self.key_gen_confirmation_callback(next_epoch),
                            expires_at: deadlines.as_ref().map(|deadlines| deadlines.confirm),
                        }));
                        *status = KeyGenConfirmation::Confirmed(Arc::new(key_share));
                    }
                    Err(err) => {
                        // Finalization failures are unexpected, as all secret
                        // shares were already verified.
                        return fail_rollover!(state, block, next_epoch, group.id(), err);
                    }
                }
            }
        }

        (state, commands)
    }

    /// Drives the epoch-rollover machine forward on the block clock: once
    /// the block reaches the rollover state's target epoch, stages the
    /// epoch (rolling `active_epoch` forward) if it was ready, then triggers
    /// a fresh key generation for whichever epoch is actually due now -
    /// abandoning (and queuing for pruning) whatever attempt was in flight for
    /// a now-stale target.
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
            state.active_epoch = EpochId::Number {
                number: target_epoch_number,
            };
        }

        // Whatever key generation was in flight for the stale target is being
        // abandoned in favor of the epoch actually due now. Make sure to prune
        // the key gen secrets (in case they weren't already pruned).
        if let Some(group_id) = state.rollover.group_id() {
            state = state.and_prune(block, Prune::KeyGenSecrets { group_id });
        }

        // Reap any old participating epochs for which there are no more
        // signing ceremonies. This runs linearly through the entire signing
        // ceremonies, but since it happens only once per rollover, we are OK
        // with the performance hit.
        let oldest_epoch = state
            .signing
            .values()
            .map(|signing| signing.packet().epoch())
            .fold(state.active_epoch, EpochId::min);
        let reaped_epochs = split_off_front(&mut state.epochs, &oldest_epoch);
        let state = reaped_epochs.values().fold(state, |state, reaped| {
            state.and_prune(
                block,
                Prune::GroupNonces {
                    group_id: reaped.group.id(),
                },
            )
        });

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
                if block >= deadlines.response && !unresponded.is_empty() {
                    // There are unresponded complaints past the response deadline,
                    // exclude all participants that failed to respond.
                    Some((*next_epoch, group.also_exclude(unresponded)))
                } else if block >= deadlines.confirm {
                    // There are missing confirmations past the confirmation
                    // deadline, exclude participants that did not confirm.
                    let excluded = group.exclude_all_others(confirmations);
                    Some((*next_epoch, excluded))
                } else {
                    None
                }
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
            (
                State {
                    rollover: RolloverState::WaitingForSetup {
                        next_epoch,
                        group,
                        poap,
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
            (
                State {
                    rollover: RolloverState::CollectingCommitments {
                        next_epoch,
                        group: participants.group(),
                        secrets: None,
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
            RolloverState::WaitingForSetup { next_epoch, .. }
            | RolloverState::CollectingCommitments { next_epoch, .. }
            | RolloverState::CollectingShares { next_epoch, .. }
            | RolloverState::CollectingConfirmations { next_epoch, .. } => Some(*next_epoch),
            RolloverState::Halted => None,
        }
    }

    /// The ID of the group actively being generated for [`Self::next_epoch`],
    /// if one has been created yet.
    fn group_id(&self) -> Option<B256> {
        match self {
            RolloverState::WaitingForGenesis
            | RolloverState::Halted
            | RolloverState::EpochStaged { .. }
            | RolloverState::EpochSkipped { .. } => None,
            RolloverState::WaitingForSetup { group, .. }
            | RolloverState::CollectingCommitments { group, .. }
            | RolloverState::CollectingShares { group, .. }
            | RolloverState::CollectingConfirmations { group, .. } => Some(group.id()),
            RolloverState::SigningRollover { group_id, .. } => Some(*group_id),
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
// call site before scheduling the resolved group's secrets for pruning.
macro_rules! fail_rollover {
    ($state:ident, $block:expr, $next_epoch:expr, $group_id:expr, $err:expr) => {{
        let (block, next_epoch, group_id, err) = ($block, $next_epoch, $group_id, $err);
        (
            State {
                rollover: rollover_failure(next_epoch, err),
                ..($state)
            }
            .and_prune(block, Prune::KeyGenSecrets { group_id }),
            Vec::new(),
        )
    }};
}
use fail_rollover;

/// Splits off the front of a B-tree map up to (but not including) the provided
/// key.
fn split_off_front<K, V>(map: &mut BTreeMap<K, V>, key: &K) -> BTreeMap<K, V>
where
    K: Ord,
{
    // `BTreeMap` provides an API to split the back off of a B-tree, removing
    // all items with key **greater than or equal to** the provided value. We
    // can use this and just swap our mutable reference with the result.
    let mut rest = map.split_off(key);
    mem::swap(map, &mut rest);
    rest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_off_front_splits_strictly_below_key() {
        let mut map = BTreeMap::from([(1, "a"), (2, "b"), (3, "c"), (4, "d"), (5, "e")]);
        let front = split_off_front(&mut map, &3);

        assert_eq!(front, BTreeMap::from([(1, "a"), (2, "b")]));
        assert_eq!(map, BTreeMap::from([(3, "c"), (4, "d"), (5, "e")]));
    }
}
