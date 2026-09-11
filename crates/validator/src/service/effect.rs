//! The validator effect system and its handler.

use crate::{
    bindings,
    frost::{
        self,
        keygen::{KeyShare, Secrets},
        preprocess::Nonces,
    },
    metrics::{self, EffectKind, Outcome},
    secrets::{SecretStore, nonces::NonceGenerator, store::RetainedGroups},
};
use alloy::primitives::{Address, B256};
use safenet_core::{effects::EffectHandler, index::BlockStatus};
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
    /// retained by the state machine as of `block`, scheduling all secret
    /// material belonging to other groups for deletion. A key share starts or
    /// retains a nonce generator; `None` retains the group's DKG secrets
    /// without running one.
    ReconcileGroupSecrets {
        block: u64,
        groups: BTreeMap<B256, Option<Arc<KeyShare>>>,
    },
}

impl Effect {
    /// Returns the stable label value used to identify this effect in metrics.
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
            Effect::ReconcileGroupSecrets { block, groups } => {
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
                let retained = RetainedGroups {
                    nonces: keygen
                        .iter()
                        .copied()
                        .chain(nonces.keys().copied())
                        .collect(),
                    keygen,
                };

                // Hold the generator lock across scheduling so that the
                // persisted schedules and the process-local generators are
                // reconciled in the same order as each other.
                let mut generator = self.nonce_generator.lock().await;
                if !self
                    .secrets
                    .schedule_group_secrets_deletion(block, &retained)
                    .await?
                {
                    // A reconciliation older than the last accepted one no
                    // longer describes the groups being tracked, so leave both
                    // the schedules and the generators as they are.
                    tracing::debug!(block, "ignoring outdated group secret reconciliation");
                    return Ok(Resume::Noop);
                }

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
        let (resume, result) = match self.try_perform_effect(effect.clone()).await {
            Ok(resume) => (resume, Outcome::Success),
            Err(err) => {
                tracing::warn!(?effect, %err, "failed to perform effect");
                (Resume::Noop, Outcome::Failure)
            }
        };
        metrics::effects_total(kind, result).increment(1);
        resume
    }

    async fn housekeeping(&self, status: BlockStatus) {
        // Only secrets scheduled by a reconciliation at or before the snapshot
        // boundary are collected, so nothing the state machine could still roll
        // back to is deleted here.
        let result = match self.secrets.prune_scheduled_secrets(status.safe).await {
            Ok(pruned) => {
                if pruned.keygen > 0 || pruned.nonces > 0 {
                    tracing::debug!(
                        block = status.latest,
                        safe = status.safe,
                        keygen = pruned.keygen,
                        nonces = pruned.nonces,
                        "pruned scheduled group secrets"
                    );
                }
                Outcome::Success
            }
            Err(err) => {
                // A later block retries the collection, and a schedule that was
                // not collected stays valid until it is.
                tracing::warn!(
                    block = status.latest,
                    safe = status.safe,
                    %err,
                    "failed to prune scheduled secrets"
                );
                Outcome::Failure
            }
        };
        metrics::housekeeping_total(result).increment(1);
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
