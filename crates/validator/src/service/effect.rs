//! The validator effect system and its handler.

use crate::{
    bindings,
    frost::{
        self,
        keygen::{KeyShare, Secrets},
        preprocess::Nonces,
    },
    secrets::SecretStore,
    service::nonce_generator::NonceGenerator,
};
use alloy::primitives::{Address, B256};
use safenet_core::effects::EffectHandler;
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    sync::Arc,
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
    /// Start eagerly generating nonce chunks for a group. Idempotent within a
    /// process.
    StartNonceGeneration {
        group_id: B256,
        key_share: Arc<KeyShare>,
    },
    /// Take the next nonce tree from a group's generator stream and persist it.
    NonceTree { group_id: B256 },
    /// Reveal this validator's nonce commitment at `(root, offset)`.
    RevealNonceCommitments {
        signature_id: B256,
        message: B256,
        root: B256,
        offset: u64,
    },
    /// Use this validator's own nonce at `(root, offset)`.
    /// Once the nonce is taken, it is burned and can no longer be used.
    UseNonce {
        message: B256,
        root: B256,
        offset: u64,
    },
    /// Prune a resolved group's keygen secrets.
    PruneKeyGenSecrets { group_id: B256 },
    /// Prune a retired group's registered nonce trees.
    PruneGroupNonces { group_id: B256 },
}

/// The result of performing an [`Effect`], resumed into the state machine.
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
    /// Resume with the nonce commitment revealed by a
    /// [`Effect::RevealNonceCommitments`].
    NonceCommitments {
        signature_id: B256,
        message: B256,
        nonces: bindings::SignNonces,
        proof: Vec<B256>,
    },
    /// Resume with the nonce burned by [`Effect::UseNonce`].
    Nonce { message: B256, nonces: Box<Nonces> },
}

/// Performs the validator's [`Effect`]s, resuming with a [`Resume`].
pub struct Handler {
    /// The account of the running validator.
    pub account: Address,
    /// The secret store containing randomly generated secrets.
    pub secrets: SecretStore,
    /// Process-local streams that eagerly generate nonce chunks by group.
    nonce_generator: NonceGenerator,
}

impl Handler {
    /// Creates an effect handler with no active nonce generator streams.
    pub fn new(account: Address, secrets: SecretStore) -> Self {
        Self {
            account,
            secrets,
            nonce_generator: NonceGenerator::new(),
        }
    }

    async fn try_perform_effect(&self, effect: Effect) -> Result<Resume, InternalError> {
        match effect {
            Effect::KeyGenSetup {
                group_id,
                count,
                threshold,
            } => {
                let secrets = {
                    let mut rng = rand::thread_rng();
                    frost::keygen::setup(&mut rng, self.account, count, threshold)?
                };
                let stored = self
                    .secrets
                    .store_keygen_secrets(group_id, self.account, secrets)
                    .await?;
                Ok(Resume::Setup {
                    group_id,
                    secrets: Box::new(stored),
                })
            }
            Effect::StartNonceGeneration {
                group_id,
                key_share,
            } => {
                self.nonce_generator.start(group_id, key_share)?;
                Ok(Resume::Noop)
            }
            Effect::NonceTree { group_id } => {
                let Some(generated) = self.nonce_generator.next(group_id).await? else {
                    tracing::debug!(%group_id, "nonce chunk request already running; ignoring duplicate effect");
                    return Ok(Resume::Noop);
                };
                let (nonce_chunk, _request) = generated.into_parts();
                let commitment = self
                    .secrets
                    .register_nonces_chunk(group_id, nonce_chunk)
                    .await?;
                Ok(Resume::NonceTree {
                    group_id,
                    commitment,
                })
            }
            Effect::RevealNonceCommitments {
                signature_id,
                message,
                root,
                offset,
            } => {
                let result = self
                    .secrets
                    .nonces_reveal(root, offset)
                    .await?
                    .map(|(nonces, proof)| Resume::NonceCommitments {
                        signature_id,
                        message,
                        nonces,
                        proof,
                    })
                    // The root is not stored, the nonce was already used, or
                    // the group was pruned; nothing to reveal.
                    .unwrap_or(Resume::Noop);
                Ok(result)
            }
            Effect::UseNonce {
                message,
                root,
                offset,
            } => {
                let result = self
                    .secrets
                    .take_nonce(root, offset)
                    .await?
                    .map(|nonces| Resume::Nonce {
                        message,
                        nonces: Box::new(nonces),
                    })
                    // The nonce was already burned, for example by a reorg
                    // replaying this effect; gracefully no-op instead of
                    // producing a duplicate signature share.
                    .unwrap_or(Resume::Noop);
                Ok(result)
            }
            Effect::PruneKeyGenSecrets { group_id } => {
                self.secrets.prune_keygen_secrets(group_id).await?;
                Ok(Resume::Noop)
            }
            Effect::PruneGroupNonces { group_id } => {
                self.nonce_generator.stop(group_id).await;
                self.secrets.prune_group_nonces(group_id).await?;
                Ok(Resume::Noop)
            }
        }
    }
}

impl EffectHandler<Effect, Resume> for Handler {
    async fn perform_effect(&self, effect: Effect) -> Resume {
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
