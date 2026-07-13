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
use safenet_core::state::EffectHandler;
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    sync::Arc,
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
    /// Link a registered nonce tree (identified by its `root` commitment) to
    /// the onchain sequence `chunk` it was assigned.
    LinkNonceTree {
        group_id: B256,
        chunk: u64,
        root: B256,
    },
    /// Reveal this validator's nonce commitment for the signing round at
    /// `sequence`.
    RevealNonceCommitments {
        group_id: B256,
        signature_id: B256,
        message: B256,
        sequence: u64,
    },
    /// Use this validator's own nonce for the signing round at `sequence`.
    /// Once the nonce is taken, it is burned and can no longer be used.
    UseNonce {
        group_id: B256,
        message: B256,
        sequence: u64,
    },
    /// Check that at least [`NONCE_TOPUP_THRESHOLD`] nonces remain usable for
    /// `key_share`'s group from `(chunk, offset)` onward, generating and
    /// registering a fresh chunk if not.
    TopupNonces {
        group_id: B256,
        key_share: Arc<KeyShare>,
        sequence: u64,
    },
    /// Prune a resolved group's keygen secrets.
    PruneKeyGenSecrets { group_id: B256 },
    /// Prune a retired group's registered nonce trees.
    PruneGroupNonces { group_id: B256 },
}

/// The remaining usable nonce count, per participating group, below which a
/// fresh nonce chunk is generated and registered.
const NONCE_TOPUP_THRESHOLD: u64 = 100;

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
            } => self.sample_nonces(group_id, &key_share).await,
            Effect::LinkNonceTree {
                group_id,
                chunk,
                root,
            } => {
                self.secrets
                    .link_nonces_chunk(group_id, self.account, chunk, root)
                    .await?;
                Ok(Resume::Noop)
            }
            Effect::RevealNonceCommitments {
                group_id,
                signature_id,
                message,
                sequence,
            } => {
                let (chunk, offset) = frost::preprocess::decode_sequence(sequence);
                let result = self
                    .secrets
                    .nonces_reveal(group_id, self.account, chunk, offset)
                    .await?
                    .map(|(nonces, proof)| Resume::NonceCommitments {
                        signature_id,
                        message,
                        nonces,
                        proof,
                    })
                    // The nonce was not generated, used up, or the tree isn't
                    // linked yet; nothing to reveal.
                    .unwrap_or(Resume::Noop);
                Ok(result)
            }
            Effect::UseNonce {
                group_id,
                message,
                sequence,
            } => {
                let (chunk, offset) = frost::preprocess::decode_sequence(sequence);
                let result = self
                    .secrets
                    .take_nonce(group_id, self.account, chunk, offset)
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
            Effect::TopupNonces {
                group_id,
                key_share,
                sequence,
            } => {
                let (chunk, offset) = frost::preprocess::decode_sequence(sequence);
                let available = self
                    .secrets
                    .available_nonce_count(group_id, self.account, chunk, offset)
                    .await?;
                if available >= NONCE_TOPUP_THRESHOLD {
                    return Ok(Resume::Noop);
                }
                return self.sample_nonces(group_id, &key_share).await;
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
        let mut rng = rand::thread_rng();
        let nonce_chunk = frost::preprocess::NonceChunk::generate(key_share, &mut rng)?;
        let result = self
            .secrets
            .register_nonces_chunk(group_id, self.account, nonce_chunk)
            .await?
            .map(|commitment| Resume::NonceTree {
                group_id,
                commitment,
            })
            // There is already a pending nonce chunk from an earlier
            // top-up; do not register a second one.
            .unwrap_or(Resume::Noop);
        tracing::trace!(
            %group_id,
            elapsed_ms = started.elapsed().as_millis(),
            "completed nonce tree sampling effect"
        );
        Ok(result)
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
