use super::{RolloverState, State, Transition};
use crate::{
    bindings::Coordinator,
    consensus::epoch::EpochId,
    frost::keygen::Secrets,
    service::{Action, Effect},
};
use alloy::primitives::B256;
use safenet_core::state::{Command, Commands};

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
}
