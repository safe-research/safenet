//! The validator effect system and its handler.

use crate::{frost, merkle::MerkleRoot, secrets::SecretStore, service::Action};
use alloy::primitives::{Address, B256};
use safenet_core::state::EffectHandler;
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// An impure operation the state transition asks the handler to perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Build a key gen commitment action.
    BuildKeyGenCommitment {
        id: B256,
        participants: MerkleRoot,
        count: u16,
        threshold: u16,
        context: B256,
        poap: Vec<B256>,
        expires_at: Option<u64>,
    },
}

/// The result of performing an [`Effect`], resumed into the state machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Resume {
    /// An effect that does not require resuming.
    #[default]
    Noop,
    /// Resume by forwarding a resolved action.
    Action(Box<Action>),
}

/// Performs the validator's [`Effect`]s, resuming with a [`Resume`].
pub struct Handler {
    /// The account of the running validator.
    pub account: Address,
    /// The secret store containing randomly generated secrets.
    pub secrets: SecretStore,
}

impl Handler {
    async fn try_perform_effect(&mut self, effect: Effect) -> Result<Resume, InternalError> {
        match effect {
            Effect::BuildKeyGenCommitment {
                id,
                participants,
                count,
                threshold,
                context,
                poap,
                expires_at,
            } => {
                let mut rng = rand::thread_rng();
                let setup = frost::keygen::setup(&mut rng, self.account, count, threshold)?;
                let stored = self
                    .secrets
                    .store_keygen_secrets(id, self.account, &setup.secrets)
                    .await?;
                if !stored {
                    tracing::info!(group_id = %id, "not resubmitting already created keygen commitments");
                    return Ok(Resume::Noop);
                }

                Ok(Resume::Action(Box::new(Action::KeyGenAndCommit {
                    participants,
                    count,
                    threshold,
                    context,
                    poap,
                    commitment: setup.commitment,
                    expires_at,
                })))
            }
        }
    }
}

impl EffectHandler<Effect, Resume> for Handler {
    async fn perform_effect(&mut self, effect: Effect) -> Resume {
        match self.try_perform_effect(effect.clone()).await {
            Ok(resume) => resume,
            Err(err) => {
                tracing::warn!(?effect, %err, "failed to perform effect");
                Resume::Noop
            }
        }
    }
}

/// An internal error used for logging failed effects.
#[derive(Debug)]
struct InternalError(String);

impl Display for InternalError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<E> From<E> for InternalError
where
    E: Error,
{
    fn from(value: E) -> Self {
        Self(value.to_string())
    }
}
