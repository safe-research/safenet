//! The snapshotted validator state.

// Right now, we only have a single field in our state but this is expected to
// change. To avoid a large swath of changes when that happens, use `..state`
// splat everywhere and silence the clippy lint. This will be removed once the
// validator state gets new fields.
#![expect(clippy::needless_update)]

mod keygen;

use crate::{
    bindings::Coordinator,
    consensus::{
        epoch::EpochId,
        group::{Group, ParticipantSet},
    },
    frost::keygen::{
        GroupCommitments, KeyShare, PublicKeyShare, Secrets, SharingState, VerifiedCommitment,
        VerifiedShare,
    },
    service::{Action, Effect, Event, Resume},
};
use alloy::primitives::{Address, B256};
use safenet_core::state::{Commands, Message, StateTransition};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
};

/// The complete snapshotted validator state.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct State {
    /// The epoch-rollover / DKG state machine.
    rollover: RolloverState,
}

/// The epoch-rollover / DKG state machine. Each active variant carries the
/// group it is generating.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
enum RolloverState {
    /// Idle before the genesis group's DKG has been triggered.
    #[default]
    WaitingForGenesis,
    /// The rollover has halted in an unrecoverable way.
    ///
    /// This can either be an unrecoverable error during DKG or we have reached
    /// the heat death of the universe and there are no more epochs.
    Halted,
    /// The key generation for `next_epoch` was skipped because too few
    /// participants took part for the group to be safe.
    EpochSkipped {
        /// The epoch whose key generation was skipped.
        next_epoch: NonZeroU64,
    },
    /// This validator is participating in the group's key generation and is
    /// waiting for the [`Effect::KeyGenSetup`] effect to complete before the
    /// commitment can be published onchain.
    WaitingForSetup {
        /// The epoch this group will serve.
        next_epoch: EpochId,
        /// The group being generated.
        group: Group,
        /// This validator's PoAP Merkle proof.
        poap: Vec<B256>,
        /// The block by which the commitment round must complete. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// A key generation is underway and the group's commitments are being
    /// collected onchain.
    CollectingCommitments {
        /// The epoch this group will serve.
        next_epoch: EpochId,
        /// The group being generated.
        group: Group,
        /// Key generation secrets, or `None` if not participating.
        secrets: Option<Box<Secrets>>,
        /// Verified commitments received from peers so far, keyed by
        /// participant.
        commitments: BTreeMap<Address, VerifiedCommitment>,
        /// The block by which the commitment round must complete. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// Every participant has committed and the group's secret shares are
    /// being collected onchain.
    CollectingShares {
        /// The epoch this group will serve.
        next_epoch: EpochId,
        /// The group being generated.
        group: Group,
        /// This validator's participation.
        participation: Box<KeyGenParticipation>,
        /// Verified participant public key shares.
        public_keys: BTreeMap<Address, PublicKeyShare>,
        /// Verified secret shares received from peers so far, keyed by
        /// participant.
        shares: BTreeMap<Address, VerifiedShare>,
        /// The block by which the secret-share round must complete. `None` to
        /// indicate that there is no deadline.
        deadline: Option<u64>,
    },
    /// Every participant has submitted a secret share (valid or not) and the
    /// group's confirmations are being collected onchain.
    CollectingConfirmations {
        /// The epoch this group will serve.
        next_epoch: EpochId,
        /// The group being generated.
        group: Group,
        /// This validator's participation.
        participation: Box<KeyGenParticipation>,
        /// The status of the confirmation for the current validator.
        status: KeyGenConfirmation,
        /// Participants that have confirmed so far.
        confirmations: BTreeSet<Address>,
        /// The confirmation deadlines for the confirmation collection phase.
        /// `None` to indicate that there is no deadline.
        deadlines: Option<ConfirmationDeadlines>,
    },
}

/// The keygen participation status for the validator.
#[derive(Clone, Debug, Deserialize, Serialize)]
enum KeyGenParticipation {
    /// The validator is an active participant that is part of the keygen
    /// ceremony and holds its keygen sharing state.
    Participating(SharingState),
    /// The validator is an observer to the ceremony, and keeps track of the
    /// public commitments needed to verify publicly revealed shares in case of
    /// complaints as well as public participant public key shares.
    Observing(GroupCommitments),
}

/// The keygen secret share confirmation status.
#[derive(Clone, Debug, Deserialize, Serialize)]
enum KeyGenConfirmation {
    /// Keygen confirmation is being observed.
    Observing,
    /// Secret shares are still being collected, as there were some shares that
    /// could not be verified.
    Collecting(BTreeMap<Address, VerifiedShare>),
    /// All secret shares have been collected and a secret key share has been
    /// constructed.
    Confirmed(Box<KeyShare>),
}

/// The deadlines for collecting confirmations.
#[derive(Clone, Debug, Deserialize, Serialize)]
struct ConfirmationDeadlines {
    /// The deadline to receive the last complaint.
    complain: u64,
    /// The deadline to receive the last complaint response.
    response: u64,
    /// The deadline to receive the last confirmation.
    confirm: u64,
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
    /// The number of blocks a distributed key generation ceremony may run
    /// before timing out.
    pub key_gen_timeout: NonZeroU64,
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
                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenCommitted(event)) => {
                    self.handle_key_gen_committed(state, log.block, &event)
                }
                Event::Coordinator(Coordinator::CoordinatorEvents::KeyGenSecretShared(event)) => {
                    self.handle_key_gen_secret_shared(state, log.block, &event)
                }
                // The remaining events are wired in as their handlers land.
                _ => (state, Vec::new()),
            },
            // No block-driven or effectful transitions are wired in yet.
            Message::NewBlock(_) => (state, Vec::new()),
            Message::Resume(result) => match result {
                Resume::Noop => (state, Vec::new()),
                Resume::Setup { group_id, secrets } => {
                    self.handle_key_gen_setup(state, group_id, secrets)
                }
            },
        }
    }
}
