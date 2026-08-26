//! The validator effect system and its handler.

use crate::{
    bindings,
    frost::{
        self,
        keygen::{KeyShare, Secrets},
        preprocess::Nonces,
    },
    metrics::{self, EffectKind, Outcome},
    secrets::{SecretStore, nonces::NonceGenerator},
};
use alloy::primitives::{Address, B256};
use safenet_core::effects::EffectHandler;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt::{self, Display, Formatter},
    sync::Arc,
};
use tokio::sync::Mutex;

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
    /// Reconcile the process-local and persisted secrets with the groups
    /// retained by the state machine, dropping all secret material belonging to
    /// other groups. A key share starts or retains a nonce generator; `None`
    /// retains the group's DKG secrets without running one.
    ReconcileGroupSecrets {
        groups: BTreeMap<B256, Option<Arc<KeyShare>>>,
    },
}

impl Effect {
    fn metric_kind(&self) -> EffectKind {
        match self {
            Self::KeyGenSetup { .. } => EffectKind::KeyGenSetup,
            Self::StartNonceGeneration { .. } => EffectKind::StartNonceGeneration,
            Self::NonceTree { .. } => EffectKind::NonceTree,
            Self::RevealNonceCommitments { .. } => EffectKind::RevealNonceCommitments,
            Self::UseNonce { .. } => EffectKind::UseNonce,
            Self::ReconcileGroupSecrets { .. } => EffectKind::ReconcileGroupSecrets,
        }
    }
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
    account: Address,
    /// The secret store containing randomly generated secrets.
    secrets: SecretStore,
    /// Process-local streams that eagerly generate nonce chunks by group.
    nonce_generator: Mutex<NonceGenerator>,
}

impl Handler {
    /// Creates an effect handler with no active nonce generator streams.
    pub fn new(account: Address, secrets: SecretStore) -> Self {
        Self {
            account,
            secrets,
            nonce_generator: Mutex::new(NonceGenerator::new()),
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
                self.nonce_generator
                    .lock()
                    .await
                    .start(group_id, key_share)?;
                Ok(Resume::Noop)
            }
            Effect::NonceTree { group_id } => {
                let next = {
                    let generator = self.nonce_generator.lock().await;
                    generator.next(group_id)
                };
                let Some(nonce_chunk) = next.await? else {
                    tracing::debug!(%group_id, "nonce chunk request already running; ignoring duplicate effect");
                    return Ok(Resume::Noop);
                };

                let commitment = self
                    .secrets
                    .register_nonces_chunk(group_id, self.account, nonce_chunk)
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
            } => Ok(self
                .secrets
                .nonces_reveal(root, offset)
                .await?
                .map(|(nonces, proof)| Resume::NonceCommitments {
                    signature_id,
                    message,
                    nonces,
                    proof,
                })
                .unwrap_or(Resume::Noop)),
            Effect::UseNonce {
                message,
                root,
                offset,
            } => Ok(self
                .secrets
                .take_nonce(root, offset)
                .await?
                .map(|nonces| Resume::Nonce {
                    message,
                    nonces: Box::new(nonces),
                })
                .unwrap_or(Resume::Noop)),
            Effect::ReconcileGroupSecrets { groups } => {
                // We only need to keep keygen secrets for groups that are still
                // in DKG and do not yet have a key share.
                // The process-local nonce generator, however, can only run for
                // groups that currently have a key share to generate with.
                let (keygen, nonces) = groups.into_iter().fold(
                    (BTreeSet::new(), BTreeMap::new()),
                    |(mut keygen, mut nonces), (group_id, key_share)| {
                        if let Some(key_share) = key_share {
                            nonces.insert(group_id, key_share);
                        } else {
                            keygen.insert(group_id);
                        }
                        (keygen, nonces)
                    },
                );

                // Retaining nonces for all groups tracked in the secret store
                // (with and without secret share) is a work around for the
                // issue that a reorg can roll a group's key share back to
                // `None` after nonces were already generated for it (e.g. a
                // restart replaying past the block where the key share was
                // confirmed), and those nonces must survive until the group
                // either re-confirms its key share or is dropped entirely.
                self.secrets
                    .retain_nonces(keygen.iter().copied().chain(nonces.keys().copied()))
                    .await?;
                self.secrets.retain_keygen_secrets(keygen).await?;

                let mut generator = self.nonce_generator.lock().await;
                generator.retain(|group_id| nonces.contains_key(group_id));
                for (group_id, key_share) in nonces {
                    generator.start(group_id, key_share)?;
                }

                Ok(Resume::Noop)
            }
        }
    }
}

impl EffectHandler<Effect, Resume> for Handler {
    async fn perform_effect(&self, effect: Effect) -> Resume {
        let kind = effect.metric_kind();
        match self.try_perform_effect(effect.clone()).await {
            Ok(resume) => {
                metrics::effect(kind, Outcome::Success);
                resume
            }
            Err(err) => {
                metrics::effect(kind, Outcome::Failure);
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

#[cfg(test)]
mod tests {
    use super::*;
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};
    use sqlx::SqlitePool;

    async fn make_handler() -> (Handler, SqlitePool) {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let secrets = SecretStore::new(pool.clone()).await.unwrap();
        (Handler::new(Address::ZERO, secrets), pool)
    }

    fn effect_counter<'a>(
        values: &'a [(
            metrics_util::CompositeKey,
            Option<::metrics::Unit>,
            Option<::metrics::SharedString>,
            DebugValue,
        )],
        effect: &str,
        result: &str,
    ) -> &'a DebugValue {
        &values
            .iter()
            .find(|(key, _, _, _)| {
                key.key().name() == "validator_effects"
                    && key
                        .key()
                        .labels()
                        .map(|label| (label.key(), label.value()))
                        .eq([("effect", effect), ("result", result)])
            })
            .unwrap()
            .3
    }

    #[test]
    fn every_effect_variant_maps_to_its_stable_kind() {
        let key_share = Arc::new(KeyShare::dummy());
        let effects = [
            (
                Effect::KeyGenSetup {
                    group_id: B256::ZERO,
                    count: 1,
                    threshold: 1,
                },
                EffectKind::KeyGenSetup,
            ),
            (
                Effect::StartNonceGeneration {
                    group_id: B256::ZERO,
                    key_share,
                },
                EffectKind::StartNonceGeneration,
            ),
            (
                Effect::NonceTree {
                    group_id: B256::ZERO,
                },
                EffectKind::NonceTree,
            ),
            (
                Effect::RevealNonceCommitments {
                    signature_id: B256::ZERO,
                    message: B256::ZERO,
                    root: B256::ZERO,
                    offset: 0,
                },
                EffectKind::RevealNonceCommitments,
            ),
            (
                Effect::UseNonce {
                    message: B256::ZERO,
                    root: B256::ZERO,
                    offset: 0,
                },
                EffectKind::UseNonce,
            ),
            (
                Effect::ReconcileGroupSecrets {
                    groups: BTreeMap::new(),
                },
                EffectKind::ReconcileGroupSecrets,
            ),
        ];

        for (effect, expected) in effects {
            assert_eq!(effect.metric_kind(), expected);
        }
    }

    #[tokio::test]
    async fn records_expected_noops_as_success_and_swallowed_errors_as_failure() {
        let recorder = DebuggingRecorder::new();
        let _guard = ::metrics::set_default_local_recorder(&recorder);

        let (handler, _pool) = make_handler().await;
        let resume = handler
            .perform_effect(Effect::UseNonce {
                message: B256::ZERO,
                root: B256::ZERO,
                offset: 0,
            })
            .await;
        assert!(matches!(resume, Resume::Noop));

        let (handler, pool) = make_handler().await;
        pool.close().await;
        let resume = handler
            .perform_effect(Effect::UseNonce {
                message: B256::ZERO,
                root: B256::ZERO,
                offset: 0,
            })
            .await;
        assert!(matches!(resume, Resume::Noop));

        let values = recorder.snapshotter().snapshot().into_vec();
        assert_eq!(
            effect_counter(&values, "use_nonce", "success"),
            &DebugValue::Counter(1)
        );
        assert_eq!(
            effect_counter(&values, "use_nonce", "failure"),
            &DebugValue::Counter(1)
        );
    }
}
