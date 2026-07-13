//! The validator service.

mod action;
mod effect;

pub use self::{
    action::Action,
    effect::{Effect, Resume},
};
use crate::{
    bindings::{Consensus, Coordinator, Oracle},
    config::ValidatorConfig,
    consensus::{
        group::{self, Epoch, ParticipantSet},
        hashing::ConsensusDomain,
    },
    secrets::{self, SecretStore},
    state::{self, State},
};
use alloy::primitives::Address;
use safenet_core::{driver::Service, watcher_events};
use sqlx::SqlitePool;

/// The validator service bundle: the state transition, effect handler and
/// action encoder that the driver runs.
pub struct ValidatorService {
    /// The account of the running validator.
    account: Address,
    /// The secret store containing keygen coeffecients and signing nonces.
    secrets: SecretStore,
    /// The genesis participant set.
    genesis: ParticipantSet,
    /// The consensus signing domain.
    consensus: ConsensusDomain,
    /// The FROST coordinator contract to submit protocol actions to.
    coordinator: Address,
    /// The validator configuration.
    config: ValidatorConfig,
}

impl ValidatorService {
    /// Creates the validator service from its machine configuration.
    pub async fn new(
        chain_id: u64,
        account: Address,
        pool: SqlitePool,
        coordinator: Address,
        config: ValidatorConfig,
    ) -> Result<Self, Error> {
        let secrets = SecretStore::new(pool).await?;
        let genesis = group::participants_set(
            &config.participants,
            Epoch::Genesis {
                salt: config.genesis_salt,
            },
        )
        .ok_or(Error::InvalidValidators)?;
        let consensus = ConsensusDomain::new(chain_id, config.consensus);

        Ok(Self {
            account,
            secrets,
            genesis,
            consensus,
            coordinator,
            config,
        })
    }
}

watcher_events! {
    /// The full event set the validator watches and dispatches on: the
    /// `Consensus` and `Coordinator` contracts plus the oracle result event.
    #[derive(Debug)]
    pub enum Event {
        Consensus(Consensus::ConsensusEvents),
        Coordinator(Coordinator::CoordinatorEvents),
        Oracle(Oracle::OracleEvents),
    }
}

impl Service for ValidatorService {
    type State = State;
    type Event = Event;

    type Transition = state::Transition;
    type Effects = effect::Handler;
    type Actions = action::Encoder;

    fn components(self) -> (Self::Transition, Self::Effects, Self::Actions) {
        let ValidatorService {
            account,
            genesis,
            secrets,
            consensus,
            coordinator,
            config,
        } = self;
        (
            state::Transition {
                account,
                genesis,
                consensus,
                config,
            },
            effect::Handler { account, secrets },
            action::Encoder { coordinator },
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Storage error initializing the secret store.
    #[error(transparent)]
    Secrets(#[from] secrets::Error),
    /// Invalid validator set.
    ///
    /// The configured validator set does not constitute a valid genesis group.
    #[error("invalid validator set: unable to form a genesis group")]
    InvalidValidators,
}
