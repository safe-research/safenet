//! The validator effect system and its handler.

use crate::{
    bindings,
    frost::{
        self,
        keygen::{KeyShare, Secrets},
        preprocess::Nonces,
    },
    secrets::SecretStore,
};
use alloy::primitives::{Address, B256};
use safenet_core::effects::EffectHandler;
use std::{
    collections::BTreeSet,
    error::Error,
    fmt::{self, Display, Formatter},
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
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
        key_share: Arc<KeyShare>,
    },
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
    /// Groups whose expensive nonce generation effect is currently running.
    nonce_generations: Mutex<BTreeSet<B256>>,
}

impl Handler {
    /// Creates an effect handler with an empty process-local generation
    /// registry.
    pub fn new(account: Address, secrets: SecretStore) -> Self {
        Self {
            account,
            secrets,
            nonce_generations: Mutex::new(BTreeSet::new()),
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
            Effect::NonceTree {
                group_id,
                key_share,
            } => {
                let Some(_generation) = self.begin_nonce_generation(group_id) else {
                    tracing::debug!(%group_id, "nonce generation already running; ignoring duplicate effect");
                    return Ok(Resume::Noop);
                };
                self.sample_nonces(group_id, &key_share).await
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
                self.secrets.prune_group_nonces(group_id).await?;
                Ok(Resume::Noop)
            }
        }
    }

    async fn sample_nonces(
        &self,
        group_id: B256,
        key_share: &KeyShare,
    ) -> Result<Resume, InternalError> {
        let started = Instant::now();
        let nonce_chunk = {
            let mut rng = rand::thread_rng();
            frost::preprocess::NonceChunk::generate(key_share, &mut rng)?
        };
        let commitment = self
            .secrets
            .register_nonces_chunk(group_id, nonce_chunk)
            .await?;
        let result = Resume::NonceTree {
            group_id,
            commitment,
        };
        tracing::trace!(
            %group_id,
            elapsed_ms = started.elapsed().as_millis(),
            "completed nonce tree sampling effect"
        );
        Ok(result)
    }

    /// Claims the process-local generation slot for `group_id`.
    fn begin_nonce_generation(&self, group_id: B256) -> Option<GenerationGuard<'_>> {
        let inserted = lock(&self.nonce_generations).insert(group_id);
        if inserted {
            Some(GenerationGuard {
                group_id,
                generations: &self.nonce_generations,
            })
        } else {
            None
        }
    }
}

/// Releases a group's nonce generation slot on success, error, or cancellation.
struct GenerationGuard<'a> {
    group_id: B256,
    generations: &'a Mutex<BTreeSet<B256>>,
}

impl Drop for GenerationGuard<'_> {
    fn drop(&mut self) {
        lock(self.generations).remove(&self.group_id);
    }
}

/// Acquires a generation-registry lock, recovering the inner set if another
/// task panicked while holding it.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
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
