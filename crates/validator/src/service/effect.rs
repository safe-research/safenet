//! The validator effect system and its handler.

use crate::{
    bindings,
    frost::{
        self,
        keygen::{KeyShare, Secrets},
        preprocess::Nonces,
    },
    secrets::{SecretStore, nonces::NonceGenerator},
};
use alloy::primitives::{Address, B256};
use safenet_core::effects::EffectHandler;
use std::{
    collections::BTreeMap,
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
    /// Sample a fresh nonce tree for `key_share` and persist it.
    ///
    /// This compatibility effect starts the group's generator if necessary,
    /// then takes its next generated chunk.
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
    /// Reveal this validator's nonce commitment at `(root, offset)`.
    #[expect(
        dead_code,
        reason = "introduced ahead of state-machine nonce integration"
    )]
    RevealNonceCommitmentsByRoot {
        signature_id: B256,
        message: B256,
        root: B256,
        offset: u64,
    },
    /// Use this validator's own nonce for the signing round at `sequence`.
    /// Once the nonce is taken, it is burned and can no longer be used.
    UseNonce {
        group_id: B256,
        message: B256,
        sequence: u64,
    },
    /// Use this validator's own nonce at `(root, offset)`.
    /// Once the nonce is taken, it is burned and can no longer be used.
    #[expect(
        dead_code,
        reason = "introduced ahead of state-machine nonce integration"
    )]
    UseNonceByRoot {
        message: B256,
        root: B256,
        offset: u64,
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
    /// Reconcile process-local and persisted secrets with the groups retained
    /// by the state machine. A key share starts or retains a nonce generator;
    /// `None` retains stored group secrets without running one.
    ReconcileGroupSecrets {
        groups: BTreeMap<B256, Option<Arc<KeyShare>>>,
    },
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
    account: Address,
    /// The secret store containing randomly generated secrets.
    secrets: SecretStore,
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
            Effect::NonceTree {
                group_id,
                key_share,
            } => {
                self.nonce_generator.start(group_id, key_share)?;
                self.next_nonce_tree(group_id).await
            }
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
            Effect::RevealNonceCommitmentsByRoot {
                signature_id,
                message,
                root,
                offset,
            } => Ok(self
                .secrets
                .nonces_reveal_by_root(root, offset)
                .await?
                .map(|(nonces, proof)| Resume::NonceCommitments {
                    signature_id,
                    message,
                    nonces,
                    proof,
                })
                .unwrap_or(Resume::Noop)),
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
            Effect::UseNonceByRoot {
                message,
                root,
                offset,
            } => Ok(self
                .secrets
                .take_nonce_by_root(root, offset)
                .await?
                .map(|nonces| Resume::Nonce {
                    message,
                    nonces: Box::new(nonces),
                })
                .unwrap_or(Resume::Noop)),
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
                self.nonce_generator.start(group_id, key_share)?;
                self.next_nonce_tree(group_id).await
            }
            Effect::PruneKeyGenSecrets { group_id } => {
                self.secrets.prune_keygen_secrets(group_id).await?;
                Ok(Resume::Noop)
            }
            Effect::ReconcileGroupSecrets { groups } => {
                self.secrets
                    .retain_group_secrets(groups.keys().copied())
                    .await?;
                self.nonce_generator.retain(|group_id| {
                    groups
                        .get(group_id)
                        .is_some_and(|key_share| key_share.is_some())
                })?;
                for (group_id, key_share) in groups {
                    let Some(key_share) = key_share else { continue };
                    self.nonce_generator.start(group_id, key_share)?;
                }
                Ok(Resume::Noop)
            }
        }
    }

    async fn next_nonce_tree(&self, group_id: B256) -> Result<Resume, InternalError> {
        let Some(nonce_chunk) = self.nonce_generator.next(group_id).await? else {
            tracing::debug!(%group_id, "nonce chunk request already running; ignoring duplicate effect");
            return Ok(Resume::Noop);
        };
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
        Ok(result)
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
