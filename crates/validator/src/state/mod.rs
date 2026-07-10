//! The snapshotted validator state.

mod keygen;

use crate::{
    bindings::Coordinator,
    consensus::{epoch::EpochId, group::ParticipantSet},
    service::{Action, Effect, Event, Resume},
};
use alloy::primitives::{Address, B256};
use safenet_core::state::{Command, Commands, Message, StateTransition};
use serde::{Deserialize, Serialize};

/// The complete snapshotted validator state.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct State {
    /// The epoch-rollover / DKG state machine.
    pub rollover: RolloverState,
}

/// The epoch-rollover / DKG state machine. Each active variant carries the
/// group it is generating.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum RolloverState {
    /// Idle before the genesis group's DKG has been triggered.
    #[default]
    WaitingForGenesis,
    /// The key generation for `next_epoch` was skipped because too few
    /// participants took part for the group to be safe.
    EpochSkipped {
        /// The epoch whose key generation was skipped.
        next_epoch: EpochId,
    },
    /// A key generation is underway and the group's commitments are being
    /// collected onchain.
    CollectingCommitments {
        /// The group being generated.
        group_id: B256,
        /// The epoch this group will serve.
        next_epoch: EpochId,
        /// The block by which the commitment round must complete. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
}

/// The pure validator state transition.
///
/// Holds the machine configuration the transition is parameterized over; the
/// group parameters it derives (identities, roots, thresholds) are all pure
/// functions of this configuration rather than snapshot state.
pub struct Transition {
    /// The account of the running validator.
    pub account: Address,
    /// The genesis participant set.
    pub genesis: ParticipantSet,
}

impl StateTransition<State> for Transition {
    type Event = Event;
    type Action = Action;
    type Effect = Effect;
    type Resume = Resume;

    fn apply_transition(
        &self,
        state: State,
        message: Message<Self::Event, Self::Resume>,
    ) -> (State, Commands<State, Self>) {
        match message {
            Message::Event(log) => match log.data {
                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGen(event)) => {
                    self.handle_genesis_key_gen(state, &event)
                }
                // The remaining events are wired in as their handlers land.
                _ => (state, Vec::new()),
            },
            // No block-driven or effectful transitions are wired in yet.
            Message::NewBlock(_) => (state, Vec::new()),
            Message::Resume(result) => match result {
                Resume::Noop => (state, Vec::new()),
                Resume::Action(action) => (state, vec![Command::Action(*action)]),
            },
        }
    }
}
