use super::{RolloverState, State, Transition};
use crate::{
    bindings::Coordinator,
    consensus::epoch::EpochId,
    frost::{
        self,
        keygen::{GroupCommitments, Secrets},
    },
    service::{Action, Effect},
    state::{ConfirmationDeadlines, KeyGenConfirmation, KeyGenParticipation},
};
use alloy::primitives::B256;
use safenet_core::state::{Command, Commands};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
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

        // Only participate in the group generation if you are part of the
        // genesis group; otherwise go straight to collecting the other
        // participants' commitments. The genesis group generation is not
        // subject to a rollover deadline.
        if let Some((group, poap)) = self.genesis.participate_as(self.account) {
            let group_id = group.id();
            let (count, threshold) = group.size();
            (
                State {
                    rollover: RolloverState::WaitingForSetup {
                        next_epoch: EpochId::Genesis,
                        group,
                        poap,
                        deadline: None,
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
                        next_epoch: EpochId::Genesis,
                        group: self.genesis.group(),
                        secrets: None,
                        commitments: BTreeMap::new(),
                        deadline: None,
                    },
                    ..state
                },
                Vec::new(),
            )
        }
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
                let deadline = deadline.map(|_| block.saturating_add(self.key_gen_timeout.get()));

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
                        return (
                            State {
                                rollover: rollover_failure(next_epoch, err),
                                ..state
                            },
                            Vec::new(),
                        );
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
                            let expires_at =
                                deadline.map(|_| block.saturating_add(self.key_gen_timeout.get()));

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
                    complain: block.saturating_add(self.key_gen_timeout.get()),
                    response: block.saturating_add(self.key_gen_timeout.get().saturating_mul(2)),
                    confirm: block.saturating_add(self.key_gen_timeout.get().saturating_mul(3)),
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
                                    expires_at: deadlines
                                        .as_ref()
                                        .map(|deadlines| deadlines.confirm),
                                }));
                                KeyGenConfirmation::Confirmed(Box::new(key_share))
                            }
                            Err(err) => {
                                // Finalization failures are unexpected, since
                                // all secret shares were already verified.
                                return (
                                    State {
                                        rollover: rollover_failure(next_epoch, err),
                                        ..state
                                    },
                                    commands,
                                );
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
        state: State,
        event: &Coordinator::KeyGenConfirmed,
    ) -> (State, Commands<State, Self>) {
        match state.rollover {
            RolloverState::CollectingConfirmations {
                next_epoch,
                group,
                participation,
                status,
                mut confirmations,
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
                                deadlines,
                            },
                            ..state
                        },
                        Vec::new(),
                    );
                }

                match next_epoch {
                    // On genesis: the group is confirmed it is ready to go.
                    EpochId::Genesis => {
                        let commands = if let KeyGenConfirmation::Confirmed(key_share) = status {
                            vec![Command::Effect(Effect::NonceTree {
                                group_id: group.id(),
                                key_share,
                            })]
                        } else {
                            Vec::new()
                        };

                        (
                            State {
                                rollover: RolloverState::EpochStaged { next_epoch },
                                ..state
                            },
                            commands,
                        )
                    }
                    _ => todo!("only genesis is supported!"),
                }
            }
            _ => (state, Vec::new()),
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
