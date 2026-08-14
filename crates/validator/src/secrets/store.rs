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

use crate::{
    bindings,
    frost::{
        keygen::Secrets,
        preprocess::{NonceChunk, Nonces},
    },
};
use alloy::{
    hex::ToHexExt,
    primitives::{Address, B256},
};
use sqlx::{QueryBuilder, Sqlite, sqlite::SqlitePool};
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
    /// An arithmetic overflow converting an integer to the database format.
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
             );

             CREATE TABLE IF NOT EXISTS nonces_chunks (
                 root     TEXT NOT NULL,
                 group_id TEXT NOT NULL,
                 address  TEXT NOT NULL,
                 PRIMARY KEY (root)
             );

             CREATE TABLE IF NOT EXISTS nonces (
                 root  TEXT    NOT NULL,
                 offs  INTEGER NOT NULL,
                 nonce TEXT    NOT NULL,
                 PRIMARY KEY (root, offs),
                 FOREIGN KEY (root) REFERENCES nonces_chunks (root) ON DELETE CASCADE
             );

             CREATE INDEX IF NOT EXISTS idx_nonces_chunks_group
                 ON nonces_chunks (group_id);",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// Persists the DKG `secrets` `me` generated for `group` and returns the
    /// secrets stored for that key.
    ///
    /// Existing secrets are **never overwritten**: a keygen commit effect
    /// reuses the retained secrets rather than resampling them, so a
    /// reorged-and-re-included commitment stays consistent with the shares the
    /// validator can still produce.
    pub async fn store_keygen_secrets(
        &self,
        group: B256,
        me: Address,
        secrets: Secrets,
    ) -> Result<Secrets, Error> {
        let stored = sqlx::query_scalar::<_, String>(
            "INSERT INTO keygen_secrets (group_id, address, secrets) VALUES (?, ?, ?)
             ON CONFLICT (group_id, address) DO UPDATE
                 SET secrets = keygen_secrets.secrets
             RETURNING secrets",
        )
        .bind(key(group))
        .bind(key(me))
        .bind(serde_json::to_string(&secrets)?)
        .fetch_one(&self.pool)
        .await?;
        let stored = serde_json::from_str(&stored)?;
        Ok(stored)
    }

    /// Deletes the DKG secrets of every group other than `groups`, reconciling
    /// the stored secrets with the groups the state machine still tracks.
    ///
    /// Specifying an empty `groups` will remove all DKG secrets.
    ///
    /// Idempotent.
    pub async fn retain_keygen_secrets(
        &self,
        groups: impl IntoIterator<Item = B256>,
    ) -> Result<(), Error> {
        self.retain_groups("keygen_secrets", groups).await
    }

    /// Persists the freshly generated preprocessing `chunk`, tagged with its
    /// owning `group` and participant, and echoes back its Merkle root.
    pub async fn register_nonces_chunk(
        &self,
        group: B256,
        me: Address,
        chunk: NonceChunk,
    ) -> Result<B256, Error> {
        let root = chunk.commitment.0;

        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO nonces_chunks (root, group_id, address) VALUES (?, ?, ?)")
            .bind(key(root))
            .bind(key(group))
            .bind(key(me))
            .execute(&mut *tx)
            .await?;
        for (offset, nonce) in chunk.nonces.into_iter().enumerate() {
            sqlx::query("INSERT INTO nonces (root, offs, nonce) VALUES (?, ?, ?)")
                .bind(key(root))
                .bind(i64::try_from(offset)?)
                .bind(serde_json::to_string(&nonce)?)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;

        Ok(root)
    }

    /// Returns the public reveal of the nonce at `(root, offset)` without
    /// removing it, or `None` when no such nonce is stored.
    ///
    /// Only [`Nonces::reveal`] information (the onchain commitments and merkle
    /// proof) is returned, never the secret nonce itself. Because this is a
    /// non-consuming read of public data, a state transition may call it
    /// repeatedly - for example to re-emit a nonce reveal after a reorg -
    /// without risking nonce reuse.
    pub async fn nonces_reveal(
        &self,
        root: B256,
        offset: u64,
    ) -> Result<Option<(bindings::SignNonces, Vec<B256>)>, Error> {
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT nonce FROM nonces
             WHERE root = ? AND offs = ?",
        )
        .bind(key(root))
        .bind(i64::try_from(offset)?)
        .fetch_optional(&self.pool)
        .await?
        .map(|nonce| serde_json::from_str::<Nonces>(&nonce))
        .transpose()?
        .map(|nonce| {
            let (nonces, proof) = nonce.reveal();
            (nonces, proof.to_vec())
        }))
    }

    /// Removes and returns the nonce at `(root, offset)`.
    ///
    /// The nonce is **deleted** from the store, so a subsequent call (for
    /// example a replay after a reorg) returns `None` and the transition
    /// gracefully no-ops instead of reusing the nonce. Deletion is permanent
    /// and not undone by a reorg; the returned nonce lives on only in the
    /// snapshot state, which a reorg is free to roll back.
    pub async fn take_nonce(&self, root: B256, offset: u64) -> Result<Option<Nonces>, Error> {
        sqlx::query_scalar::<_, String>(
            "DELETE FROM nonces
             WHERE root = ? AND offs = ?
             RETURNING nonce",
        )
        .bind(key(root))
        .bind(i64::try_from(offset)?)
        .fetch_optional(&self.pool)
        .await?
        .map(|nonce| serde_json::from_str(&nonce))
        .transpose()
        .map_err(Error::from)
    }

    /// Deletes the nonce trees of every group other than `groups` (cascading to
    /// their nonces), reconciling the stored nonces with the groups the state
    /// machine still tracks.
    ///
    /// Specifying an empty `groups` will remove all nonce trees and nonces.
    ///
    /// Idempotent.
    pub async fn retain_nonces(&self, groups: impl IntoIterator<Item = B256>) -> Result<(), Error> {
        self.retain_groups("nonces_chunks", groups).await
    }

    /// Deletes every row in `table` whose `group_id` is not one of `groups`.
    async fn retain_groups(
        &self,
        table: &'static str,
        groups: impl IntoIterator<Item = B256>,
    ) -> Result<(), Error> {
        let mut groups = groups.into_iter().peekable();
        let mut query = if groups.peek().is_none() {
            QueryBuilder::<Sqlite>::new(format!("DELETE FROM {table}"))
        } else {
            let mut query =
                QueryBuilder::<Sqlite>::new(format!("DELETE FROM {table} WHERE group_id NOT IN ("));
            let mut retained = query.separated(", ");
            for group in groups {
                retained.push_bind(key(group));
            }
            retained.push_unseparated(")");
            query
        };

        query.build().execute(&self.pool).await?;
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
    use crate::frost::{keygen, preprocess::NonceChunk};
    use alloy::primitives::address;

    const GROUP: B256 = B256::repeat_byte(0xa1);
    const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    async fn store() -> SecretStore {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        SecretStore::new(pool).await.unwrap()
    }

    fn keygen_secrets() -> keygen::Secrets {
        keygen::setup(&mut rand::thread_rng(), ME, 3, 2).unwrap()
    }

    fn nonce_chunk(size: u64) -> NonceChunk {
        NonceChunk::with_size(size, &keygen::KeyShare::dummy(), &mut rand::thread_rng()).unwrap()
    }

    async fn get_keygen_secrets(store: &SecretStore, group: B256) -> Option<Secrets> {
        sqlx::query_scalar::<_, String>(
            "SELECT secrets FROM keygen_secrets WHERE group_id = ? AND address = ?",
        )
        .bind(key(group))
        .bind(key(ME))
        .fetch_optional(&store.pool)
        .await
        .unwrap()
        .map(|secrets| serde_json::from_str(&secrets))
        .transpose()
        .unwrap()
    }

    async fn count_root_nonces(store: &SecretStore, root: B256) -> u64 {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM nonces WHERE root = ?")
            .bind(key(root))
            .fetch_one(&store.pool)
            .await
            .unwrap();
        u64::try_from(count).unwrap()
    }

    #[tokio::test]
    async fn keygen_secrets_roundtrip_and_missing() {
        let store = store().await;
        assert!(get_keygen_secrets(&store, GROUP).await.is_none());

        let secrets = keygen_secrets();
        store
            .store_keygen_secrets(GROUP, ME, secrets.clone())
            .await
            .unwrap();

        let read = get_keygen_secrets(&store, GROUP).await.unwrap();
        assert_eq!(
            serde_json::to_string(&read).unwrap(),
            serde_json::to_string(&secrets).unwrap(),
        )
    }

    #[tokio::test]
    async fn store_keygen_secrets_does_not_overwrite() {
        // A re-run of the commit effect (for example after a reorg re-includes
        // the commitment) must reuse the retained secrets, not resample them.
        let store = store().await;

        let first = keygen_secrets();
        let stored = store
            .store_keygen_secrets(GROUP, ME, first.clone())
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_string(&stored).unwrap(),
            serde_json::to_string(&first).unwrap(),
        );

        let second = keygen_secrets();
        assert_ne!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap(),
        );

        let stored = store.store_keygen_secrets(GROUP, ME, second).await.unwrap();
        assert_eq!(
            serde_json::to_string(&stored).unwrap(),
            serde_json::to_string(&first).unwrap(),
        );

        let read = get_keygen_secrets(&store, GROUP).await.unwrap();
        assert_eq!(
            serde_json::to_string(&read).unwrap(),
            serde_json::to_string(&first).unwrap(),
        );
    }

    #[tokio::test]
    async fn retain_keygen_secrets_removes_unretained_groups() {
        let store = store().await;
        let other_group = B256::repeat_byte(0xb2);
        for group in [GROUP, other_group] {
            store
                .store_keygen_secrets(group, ME, keygen_secrets())
                .await
                .unwrap();
        }

        store.retain_keygen_secrets([other_group]).await.unwrap();
        assert!(get_keygen_secrets(&store, GROUP).await.is_none());
        assert!(get_keygen_secrets(&store, other_group).await.is_some());
        // Retaining the same group again is a no-op.
        store.retain_keygen_secrets([other_group]).await.unwrap();
        assert!(get_keygen_secrets(&store, other_group).await.is_some());

        // Retaining no groups removes all DKG secrets.
        store.retain_keygen_secrets([]).await.unwrap();
        assert!(get_keygen_secrets(&store, other_group).await.is_none());
    }

    #[tokio::test]
    async fn nonce_chunks_are_stored() {
        let store = store().await;
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(11))
            .await
            .unwrap();
        let other = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(12))
            .await
            .unwrap();
        assert_ne!(other, root);
        assert_eq!(count_root_nonces(&store, root).await, 11);
        assert_eq!(count_root_nonces(&store, other).await, 12);
    }

    #[tokio::test]
    async fn nonces_reveal_is_non_consuming() {
        let store = store().await;
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(4))
            .await
            .unwrap();

        assert!(store.nonces_reveal(root, 1).await.unwrap().is_some());
        assert!(store.nonces_reveal(root, 1).await.unwrap().is_some());
        assert_eq!(count_root_nonces(&store, root).await, 4);

        assert!(store.nonces_reveal(root, 99).await.unwrap().is_none());
        assert!(
            store
                .nonces_reveal(B256::repeat_byte(7), 1)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn take_nonce_removes_it_permanently() {
        let store = store().await;
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(4))
            .await
            .unwrap();

        assert!(store.take_nonce(root, 2).await.unwrap().is_some());
        assert!(store.take_nonce(root, 2).await.unwrap().is_none());
        assert_eq!(count_root_nonces(&store, root).await, 3);

        assert!(store.nonces_reveal(root, 0).await.unwrap().is_some());
        assert!(store.take_nonce(root, 0).await.unwrap().is_some());
        assert!(store.nonces_reveal(root, 0).await.unwrap().is_none());
        assert_eq!(count_root_nonces(&store, root).await, 2);

        assert!(
            store
                .take_nonce(B256::repeat_byte(7), 0)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(count_root_nonces(&store, root).await, 2);
    }
}
