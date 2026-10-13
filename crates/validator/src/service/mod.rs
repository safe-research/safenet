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
use safenet_core::{driver::Service, provider::Provider, watcher_events};
use sqlx::SqlitePool;

/// The validator service bundle: the state transition, effect handler and
/// action encoder that the driver runs.
pub struct ValidatorService {
    /// The Ethereum RPC provider for the service.
    provider: Provider,
    /// The account of the running validator.
    account: Address,
    /// The secret store containing keygen coeffecients and signing nonces.
    secrets: SecretStore,
    /// The genesis participant set.
    genesis: ParticipantSet,
    /// The FROST coordinator contract to submit protocol actions to.
    coordinator: Address,
    /// The validator configuration.
    config: ValidatorConfig,
}

impl ValidatorService {
    /// Creates the validator service from its machine configuration.
    pub async fn new(
        provider: Provider,
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
        tracing::info!(genesis_group_id = %genesis.group().id(), "configured genesis group id");

        Ok(Self {
            provider,
            account,
            secrets,
            genesis,
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
    type Action = Action;
    type Effect = Effect;
    type Resume = Resume;

    type Transition = state::Transition;
    type Initializer = state::Initializer;
    type Effects = effect::Handler;
    type Actions = action::Encoder;

    fn components(
        self,
    ) -> (
        Self::Transition,
        Self::Initializer,
        Self::Effects,
        Self::Actions,
    ) {
        let ValidatorService {
            provider,
            account,
            genesis,
            secrets,
            coordinator,
            config,
        } = self;
        let consensus_address = config.consensus;
        let blocks_per_epoch = config.blocks_per_epoch;
        let genesis_group_id = genesis.group().id();
        let consensus = ConsensusDomain::new(provider.chain_id(), consensus_address);
        (
            state::Transition {
                account,
                genesis,
                consensus,
                config,
            },
            state::Initializer {
                provider,
                consensus: consensus_address,
                coordinator,
                genesis_group_id,
                blocks_per_epoch,
            },
            effect::Handler::new(account, secrets),
            action::Encoder {
                coordinator,
                consensus: consensus_address,
            },
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Storage error initializing the secret store.
    #[error(transparent)]
    Secrets(#[from] secrets::store::Error),
    /// Invalid validator set.
    ///
    /// The configured validator set does not constitute a valid genesis group.
    #[error("invalid validator set: unable to form a genesis group")]
    InvalidValidators,
}
