//! The separate, reorg-resistant store for locally-generated random secrets.
//!
//! Two kinds of secret are sampled locally and then committed to onchain, and
//! neither may live in the reorg-aware snapshot state on its own: the **DKG
//! polynomial secrets** (a participant's random coefficients and ECDH
//! encryption key) and the **FROST signing nonces**. A reorg that rolled either
//! back while the transaction committing to it is re-included on the reorged
//! chain would strand a keygen (the validator could no longer produce the
//! matching shares) or risk reusing a nonce (which leaks the signing share).
//!
//! This store therefore lives in the shared [`SqlitePool`] but is deliberately
//! **not** rolled back on reorg. It is reached only through the validator's
//! effect handler, and its two kinds of secret are handled differently:
//!
//! - **DKG secrets** are reused (not resampled) when already present, so a
//!   reorged-and-re-included commitment stays consistent with the shares the
//!   validator can still produce. They are pruned once the keygen resolves.
//! - **Nonces** are handed out exactly once and are *removed* from the store
//!   in order to prevent accidental reuse. Unused nonces persist so a
//!   re-included `preprocess` commitment can still be signed against, and are
//!   pruned when the owning group retires.

#![cfg_attr(not(test), expect(dead_code))]

use crate::frost::keygen::Secrets;
use alloy::{
    hex::ToHexExt,
    primitives::{Address, B256},
};
use sqlx::sqlite::SqlitePool;
use std::num::TryFromIntError;

/// Error produced by the [`SecretStore`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A database operation failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// A secret could not be serialized or deserialized.
    #[error("failed to serialize or deserialize a secret")]
    Serialization(#[from] serde_json::Error),
    /// An arithmetic overflow converting a chunk or offset to or from the
    /// database integer type.
    #[error("integer conversion overflow")]
    Overflow,
}

impl From<TryFromIntError> for Error {
    fn from(_: TryFromIntError) -> Self {
        Self::Overflow
    }
}

/// SQLite-backed store for locally-generated random secrets, over the shared
/// pool. Unlike the snapshot store, it is never rolled back on reorg.
pub struct SecretStore {
    pool: SqlitePool,
}

impl SecretStore {
    /// Creates the store backed by `pool`, creating its tables if absent.
    pub async fn new(pool: SqlitePool) -> Result<Self, Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS keygen_secrets (
                 group_id TEXT NOT NULL,
                 address  TEXT NOT NULL,
                 secrets  TEXT NOT NULL,
                 PRIMARY KEY (group_id, address)
             );",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// Returns the DKG secrets `me` generated for `group`, or `None` if none
    /// are stored (they were never generated, or were pruned when the keygen
    /// resolved).
    pub async fn keygen_secrets(&self, group: B256, me: Address) -> Result<Option<Secrets>, Error> {
        sqlx::query_scalar::<_, String>(
            "SELECT secrets FROM keygen_secrets WHERE group_id = ? AND address = ?",
        )
        .bind(key(group))
        .bind(key(me))
        .fetch_optional(&self.pool)
        .await?
        .map(|secrets| serde_json::from_str(&secrets))
        .transpose()
        .map_err(Error::from)
    }

    /// Persists the DKG `secrets` `me` generated for `group`, returning whether
    /// they were stored (`true`) or dropped because secrets already exist for
    /// the key (`false`).
    ///
    /// Existing secrets are **never overwritten**: a keygen commit effect
    /// reuses the retained secrets rather than resampling them, so a
    /// reorged-and-re-included commitment stays consistent with the shares the
    /// validator can still produce.
    pub async fn store_keygen_secrets(
        &self,
        group: B256,
        me: Address,
        secrets: &Secrets,
    ) -> Result<bool, Error> {
        let inserted = sqlx::query(
            "INSERT INTO keygen_secrets (group_id, address, secrets) VALUES (?, ?, ?)
             ON CONFLICT (group_id, address) DO NOTHING",
        )
        .bind(key(group))
        .bind(key(me))
        .bind(serde_json::to_string(secrets)?)
        .execute(&self.pool)
        .await?;
        Ok(inserted.rows_affected() == 1)
    }

    /// Deletes `group`'s DKG secrets once its keygen resolves (successfully or
    /// not). Idempotent.
    pub async fn prune_keygen_secrets(&self, group: B256) -> Result<(), Error> {
        sqlx::query("DELETE FROM keygen_secrets WHERE group_id = ?")
            .bind(key(group))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Encodes a fixed-byte value (group id, nonce root or address) as its
/// lowercase hex text key, deterministic across calls.
fn key(value: impl ToHexExt) -> String {
    value.encode_hex()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frost::keygen;
    use alloy::primitives::address;

    const GROUP: B256 = B256::repeat_byte(0xa1);
    const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    async fn store() -> SecretStore {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        SecretStore::new(pool).await.unwrap()
    }

    fn keygen_secrets() -> keygen::Secrets {
        keygen::setup(&mut rand::thread_rng(), ME, 3, 2)
            .unwrap()
            .secrets
    }

    #[tokio::test]
    async fn keygen_secrets_roundtrip_and_missing() {
        let store = store().await;
        assert!(store.keygen_secrets(GROUP, ME).await.unwrap().is_none());

        let secrets = keygen_secrets();
        store
            .store_keygen_secrets(GROUP, ME, &secrets)
            .await
            .unwrap();
        assert!(store.keygen_secrets(GROUP, ME).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn store_keygen_secrets_does_not_overwrite() {
        // A re-run of the commit effect (for example after a reorg re-includes
        // the commitment) must reuse the retained secrets, not resample them.
        let store = store().await;
        let first = keygen_secrets();
        // The first store persists the secrets; the second is dropped.
        assert!(store.store_keygen_secrets(GROUP, ME, &first).await.unwrap());

        let second = keygen_secrets();
        assert!(
            !store
                .store_keygen_secrets(GROUP, ME, &second)
                .await
                .unwrap()
        );

        let stored = store.keygen_secrets(GROUP, ME).await.unwrap().unwrap();
        assert_eq!(
            serde_json::to_string(&stored).unwrap(),
            serde_json::to_string(&first).unwrap(),
        );
        assert_ne!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap(),
        );
    }

    #[tokio::test]
    async fn prune_removes_resolved_group_secrets() {
        let store = store().await;
        store
            .store_keygen_secrets(GROUP, ME, &keygen_secrets())
            .await
            .unwrap();

        store.prune_keygen_secrets(GROUP).await.unwrap();
        assert!(store.keygen_secrets(GROUP, ME).await.unwrap().is_none());
        // Pruning again is a no-op.
        store.prune_keygen_secrets(GROUP).await.unwrap();
    }
}
