use super::{RolloverState, State, Transition};
use crate::{bindings::Coordinator, consensus::epoch::EpochId, service::Effect};
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
        mut state: State,
        event: &Coordinator::KeyGen,
    ) -> (State, Commands<State, Self>) {
        let genesis = self.genesis.group();
        if state.rollover != RolloverState::WaitingForGenesis || event.gid != genesis.id() {
            return (state, Vec::new());
        }

        // The genesis group generation is not subject to a rollover deadline.
        state.rollover = RolloverState::CollectingCommitments {
            group_id: genesis.id(),
            next_epoch: EpochId::Genesis,
            deadline: None,
        };

        // Only participate in the group generation if you are part of the
        // genesis group.
        let commands = (self.genesis.participate_as(self.account))
            .map(|(group, poap)| {
                let (participants, count, threshold, context) = group.parameters();
                vec![Command::Effect(Effect::BuildKeyGenCommitment {
                    id: group.id(),
                    participants,
                    count,
                    threshold,
                    context,
                    poap,
                    expires_at: None,
                })]
            })
            .unwrap_or_default();

        (state, commands)
    }
}
