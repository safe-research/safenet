//! The validator effect system and its handler.

use crate::{
    frost::{
        self,
        keygen::{KeyShare, Secrets},
    },
    secrets::SecretStore,
};
use alloy::primitives::{Address, B256};
use safenet_core::state::EffectHandler;
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// An impure operation the state transition asks the handler to perform.
#[derive(Debug, Clone)]
pub enum Effect {
    /// Set up key generation: sample the participant's secrets, persist them
    /// to the secret store.
    KeyGenSetup {
        group_id: B256,
        count: u16,
        threshold: u16,
    },
    /// Sample a fresh nonce tree for `key_share` and persist it.
    NonceTree {
        group_id: B256,
        key_share: Box<KeyShare>,
    },
}

/// The result of performing an [`Effect`], resumed into the state machine.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Default)]
pub enum Resume {
    /// An effect that does not require resuming.
    #[default]
    Noop,
    /// Resume with the key gen commitment produced by a [`Effect::KeyGenSetup`].
    Setup {
        group_id: B256,
        secrets: Box<Secrets>,
    },
    /// Resume with the nonce tree commitment produced by a
    /// [`Effect::NonceTree`].
    NonceTree { group_id: B256, commitment: B256 },
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
            Effect::KeyGenSetup {
                group_id,
                count,
                threshold,
            } => {
                let mut rng = rand::thread_rng();
                let secrets = frost::keygen::setup(&mut rng, self.account, count, threshold)?;
                let stored = self
                    .secrets
                    .store_keygen_secrets(group_id, self.account, secrets)
                    .await?;
                Ok(Resume::Setup {
                    group_id,
                    secrets: Box::new(stored),
                })
            }
            Effect::NonceTree {
                group_id,
                key_share,
            } => {
                let mut rng = rand::thread_rng();
                let chunk = frost::preprocess::NonceChunk::generate(&key_share, &mut rng)?;
                let commitment = self
                    .secrets
                    .register_nonces_chunk(group_id, self.account, chunk)
                    .await?;
                Ok(Resume::NonceTree {
                    group_id,
                    commitment,
                })
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
