use super::{RolloverState, State, Transition};
use crate::{
    bindings::Coordinator,
    consensus::epoch::EpochId,
    frost::{
        self,
        keygen::{SecretShares, Secrets},
    },
    service::{Action, Effect},
};
use alloy::primitives::B256;
use safenet_core::state::{Command, Commands};
use std::collections::BTreeMap;

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
    pub fn handle_key_gen_setup(
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
                let (count, threshold) = group.size();

                // Only consider valid commitments; invalid ones are ignored,
                // the participant will be removed from the group on timeout.
                if let Ok(commitment) = frost::keygen::verify_commitment(
                    threshold,
                    event.participant,
                    &event.commitment,
                ) {
                    commitments.insert(event.participant, commitment);
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

                // Compute the group key from the commitments, and if part of
                // the group, the secret shares to publish onchain.
                match frost::keygen::group_key(&commitments).and_then(|group_key| {
                    let secret_shares = secrets
                        .map(|secrets| frost::keygen::generate_secret_shares(*secrets, commitments))
                        .transpose()?;
                    Ok((group_key, secret_shares))
                }) {
                    Ok((group_key, secret_shares)) => {
                        let group_id = group.id();
                        let (sharing_state, commands) = if let Some(SecretShares {
                            sharing_state,
                            share,
                        }) = secret_shares
                        {
                            (
                                Some(Box::new(sharing_state)),
                                vec![Command::Action(Action::KeyGenSecretShare {
                                    group_id,
                                    share,
                                    expires_at: deadline,
                                })],
                            )
                        } else {
                            // If we are just observing, the continue without
                            // a secret sharing state or emitting any actions.
                            (None, Vec::new())
                        };

                        (
                            State {
                                rollover: RolloverState::CollectingShares {
                                    next_epoch,
                                    group,
                                    group_key,
                                    sharing_state,
                                    shares: BTreeMap::new(),
                                    deadline,
                                },
                                ..state
                            },
                            commands,
                        )
                    }
                    Err(err) => {
                        let rollover = if let EpochId::Number { number: next_epoch } = next_epoch {
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
                        };
                        (State { rollover, ..state }, Vec::new())
                    }
                }
            }
            _ => (state, Vec::new()),
        }
    }
}
