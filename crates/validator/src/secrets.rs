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
             );

             CREATE TABLE IF NOT EXISTS nonces_chunks (
                 root     TEXT    NOT NULL,
                 group_id TEXT    NOT NULL,
                 address  TEXT    NOT NULL,
                 chunk    INTEGER DEFAULT NULL,
                 PRIMARY KEY (root)
             );

             CREATE TABLE IF NOT EXISTS nonces (
                 root  TEXT    NOT NULL,
                 offs  INTEGER NOT NULL,
                 nonce TEXT    NOT NULL,
                 PRIMARY KEY (root, offs),
                 FOREIGN KEY (root) REFERENCES nonces_chunks (root) ON DELETE CASCADE
             );

             CREATE UNIQUE INDEX IF NOT EXISTS idx_nonces_chunks_lookup
                 ON nonces_chunks (group_id, address, chunk);

             CREATE UNIQUE INDEX IF NOT EXISTS idx_nonces_chunks_unlinked
                 ON nonces_chunks (group_id, address)
                 WHERE chunk IS NULL;",
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

    /// Deletes `group`'s DKG secrets once its keygen resolves (successfully or
    /// not). Idempotent.
    pub async fn prune_keygen_secrets(&self, group: B256) -> Result<(), Error> {
        sqlx::query("DELETE FROM keygen_secrets WHERE group_id = ?")
            .bind(key(group))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Persists the freshly generated preprocessing `chunk` for `me` in
    /// `group` and returns its merkle root, or returns `None` when another
    /// unlinked chunk is already pending. At most one unlinked chunk is
    /// retained per participant and group; the existing root is deliberately
    /// not returned so callers cannot submit it under a second onchain chunk.
    /// [`link_nonces_chunk`](Self::link_nonces_chunk) associates the new root
    /// with a sequence chunk once assigned onchain.
    pub async fn register_nonces_chunk(
        &self,
        group: B256,
        me: Address,
        chunk: NonceChunk,
    ) -> Result<Option<B256>, Error> {
        let root = chunk.commitment.0;

        let mut tx = self.pool.begin().await?;
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT 1 FROM nonces_chunks
             WHERE group_id = ? AND address = ? AND chunk IS NULL",
        )
        .bind(key(group))
        .bind(key(me))
        .fetch_optional(&mut *tx)
        .await?;

        // We only allow a single pending
        if existing.is_some() {
            return Ok(None);
        };

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

        Ok(Some(root))
    }

    /// Links a registered nonce tree (by `root`) to the onchain sequence
    /// `chunk` for `me` in `group`. Replaces a previous chunk assignment, as
    /// canonical replay after a reorg may assign the commitment a different
    /// index. Errors if the root is unknown.
    pub async fn link_nonces_chunk(
        &self,
        group: B256,
        me: Address,
        chunk: u64,
        root: B256,
    ) -> Result<(), Error> {
        let chunk = i64::try_from(chunk)?;

        // A canonical replay can assign this root an index that is still
        // occupied locally by a root linked on an orphaned chain. Because we
        // may be moving a root linked to one chunk to another, we cannot simply
        // mark the old root as pending either. To work around this, we give the
        // root a negative unreachable chunk, until that one gets linked.
        let mut tx = self.pool.begin().await?;

        // First, make the old root inaccessible by giving it a negative chunk.
        sqlx::query(
            "UPDATE nonces_chunks SET chunk = -rowid
             WHERE root != ? AND group_id = ? AND address = ? AND chunk = ?",
        )
        .bind(key(root))
        .bind(key(group))
        .bind(key(me))
        .bind(chunk)
        .execute(&mut *tx)
        .await?;

        // Update the linked nonce.
        let update = sqlx::query(
            "UPDATE nonces_chunks SET chunk = ?
             WHERE root = ? AND group_id = ? AND address = ?",
        )
        .bind(chunk)
        .bind(key(root))
        .bind(key(group))
        .bind(key(me))
        .execute(&mut *tx)
        .await?;
        if update.rows_affected() == 0 {
            return Err(Error::Database(sqlx::Error::RowNotFound));
        }

        tx.commit().await?;
        Ok(())
    }

    /// Returns the public reveal of the nonce at `offset` in the tree `me`
    /// linked to sequence `chunk` in `group`, without removing it, or `None`
    /// when no such nonce is stored (never generated, already taken, or
    /// pruned).
    ///
    /// Only [`Nonces::reveal`] information (the onchain commitments and merkle
    /// proof) is returned, never the secret nonce itself. Because this is a
    /// non-consuming read of public data, a state transition may call it
    /// repeatedly - for example to re-emit a nonce reveal after a reorg -
    /// without risking nonce reuse.
    pub async fn nonces_reveal(
        &self,
        group: B256,
        me: Address,
        chunk: u64,
        offset: u64,
    ) -> Result<Option<(bindings::SignNonces, Vec<B256>)>, Error> {
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT nonce FROM nonces
             WHERE root = (
                 SELECT root FROM nonces_chunks
                 WHERE group_id = ? AND address = ? AND chunk = ?
             )
             AND offs = ?",
        )
        .bind(key(group))
        .bind(key(me))
        .bind(i64::try_from(chunk)?)
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

    /// Removes and returns the nonce at `offset` in the tree `me` linked to
    /// sequence `chunk` in `group`.
    ///
    /// The nonce is **deleted** from the store, so a subsequent call (for
    /// example a replay after a reorg) returns `None` and the transition
    /// gracefully no-ops instead of reusing the nonce. Deletion is permanent
    /// and not undone by a reorg; the returned nonce lives on only in the
    /// snapshot state, which a reorg is free to roll back.
    pub async fn take_nonce(
        &self,
        group: B256,
        me: Address,
        chunk: u64,
        offset: u64,
    ) -> Result<Option<Nonces>, Error> {
        sqlx::query_scalar::<_, String>(
            "DELETE FROM nonces
             WHERE root = (
                 SELECT root FROM nonces_chunks
                 WHERE group_id = ? AND address = ? AND chunk = ?
             )
             AND offs = ?
             RETURNING nonce",
        )
        .bind(key(group))
        .bind(key(me))
        .bind(i64::try_from(chunk)?)
        .bind(i64::try_from(offset)?)
        .fetch_optional(&self.pool)
        .await?
        .map(|nonce| serde_json::from_str(&nonce))
        .transpose()
        .map_err(Error::from)
    }

    /// Returns the number of unused nonces in the tree `me` linked to sequence
    /// `chunk` in `group`, or `0` when no such tree is linked.
    pub async fn available_nonce_count(
        &self,
        group: B256,
        me: Address,
        chunk: u64,
    ) -> Result<u64, Error> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM nonces
             WHERE root = (
                 SELECT root FROM nonces_chunks
                 WHERE group_id = ? AND address = ? AND chunk = ?
             )",
        )
        .bind(key(group))
        .bind(key(me))
        .bind(i64::try_from(chunk)?)
        .fetch_one(&self.pool)
        .await?;
        Ok(u64::try_from(count)?)
    }

    /// Deletes every nonce tree belonging to a retired `group` (cascading to its
    /// nonces). Idempotent.
    pub async fn prune_group_nonces(&self, group: B256) -> Result<(), Error> {
        sqlx::query("DELETE FROM nonces_chunks WHERE group_id = ?")
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

    #[tokio::test]
    async fn keygen_secrets_roundtrip_and_missing() {
        let store = store().await;
        assert!(store.keygen_secrets(GROUP, ME).await.unwrap().is_none());

        let secrets = keygen_secrets();
        store
            .store_keygen_secrets(GROUP, ME, secrets.clone())
            .await
            .unwrap();

        let read = store.keygen_secrets(GROUP, ME).await.unwrap().unwrap();
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

        let read = store.keygen_secrets(GROUP, ME).await.unwrap().unwrap();
        assert_eq!(
            serde_json::to_string(&read).unwrap(),
            serde_json::to_string(&first).unwrap(),
        );
    }

    #[tokio::test]
    async fn prune_removes_resolved_group_secrets() {
        let store = store().await;
        store
            .store_keygen_secrets(GROUP, ME, keygen_secrets())
            .await
            .unwrap();

        store.prune_keygen_secrets(GROUP).await.unwrap();
        assert!(store.keygen_secrets(GROUP, ME).await.unwrap().is_none());
        // Pruning again is a no-op.
        store.prune_keygen_secrets(GROUP).await.unwrap();
    }

    #[tokio::test]
    async fn nonces_chunk_link_and_count() {
        let store = store().await;
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(11))
            .await
            .unwrap()
            .unwrap();
        let reused = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(12))
            .await
            .unwrap();
        assert_eq!(reused, None);

        // Until linked to a sequence chunk, it is not addressable by chunk.
        assert_eq!(store.available_nonce_count(GROUP, ME, 0).await.unwrap(), 0);
        store.link_nonces_chunk(GROUP, ME, 0, root).await.unwrap();
        assert_eq!(store.available_nonce_count(GROUP, ME, 0).await.unwrap(), 11);

        // Once linked, a fresh chunk can be registered.
        let other = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(13))
            .await
            .unwrap()
            .unwrap();
        assert_ne!(other, root);

        // The newly registered chunk starts unlinked.
        assert_eq!(store.available_nonce_count(GROUP, ME, 0).await.unwrap(), 11);
        assert_eq!(store.available_nonce_count(GROUP, ME, 1).await.unwrap(), 0);

        // It can be linked to a chunk.
        store.link_nonces_chunk(GROUP, ME, 1, other).await.unwrap();
        assert_eq!(store.available_nonce_count(GROUP, ME, 0).await.unwrap(), 11);
        assert_eq!(store.available_nonce_count(GROUP, ME, 1).await.unwrap(), 13);
        assert_eq!(store.available_nonce_count(GROUP, ME, 2).await.unwrap(), 0);

        // Nonce assignment is idempotent.
        store.link_nonces_chunk(GROUP, ME, 0, root).await.unwrap();
        assert_eq!(store.available_nonce_count(GROUP, ME, 0).await.unwrap(), 11);
        assert_eq!(store.available_nonce_count(GROUP, ME, 1).await.unwrap(), 13);
        assert_eq!(store.available_nonce_count(GROUP, ME, 2).await.unwrap(), 0);

        // Canonical replay can reassign a root after a reorg, displacing a
        // stale assignment left by the orphaned chain.
        store.link_nonces_chunk(GROUP, ME, 1, root).await.unwrap();
        assert_eq!(store.available_nonce_count(GROUP, ME, 0).await.unwrap(), 0);
        assert_eq!(store.available_nonce_count(GROUP, ME, 1).await.unwrap(), 11);
        assert_eq!(store.available_nonce_count(GROUP, ME, 2).await.unwrap(), 0);

        // The displaced root can itself be assigned by a later canonical
        // event.
        store.link_nonces_chunk(GROUP, ME, 2, other).await.unwrap();
        assert_eq!(store.available_nonce_count(GROUP, ME, 0).await.unwrap(), 0);
        assert_eq!(store.available_nonce_count(GROUP, ME, 1).await.unwrap(), 11);
        assert_eq!(store.available_nonce_count(GROUP, ME, 2).await.unwrap(), 13);
    }

    #[tokio::test]
    async fn use_nonce_removes_it_permanently() {
        let store = store().await;
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(4))
            .await
            .unwrap()
            .unwrap();
        store.link_nonces_chunk(GROUP, ME, 0, root).await.unwrap();

        // First use returns the nonce; a second use of the same offset yields
        // `None` so the transition can gracefully no-op rather than reuse it.
        assert!(store.take_nonce(GROUP, ME, 0, 2).await.unwrap().is_some());
        assert!(store.take_nonce(GROUP, ME, 0, 2).await.unwrap().is_none());

        // The used nonce is gone; the rest of the chunk is untouched.
        assert_eq!(store.available_nonce_count(GROUP, ME, 0).await.unwrap(), 3);
        assert!(store.take_nonce(GROUP, ME, 0, 0).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn nonces_reveal_is_non_consuming() {
        let store = store().await;
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(4))
            .await
            .unwrap()
            .unwrap();
        store.link_nonces_chunk(GROUP, ME, 0, root).await.unwrap();

        // The reveal can be read repeatedly without consuming the nonce, so it
        // survives to be taken afterwards.
        assert!(
            store
                .nonces_reveal(GROUP, ME, 0, 1)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            store
                .nonces_reveal(GROUP, ME, 0, 1)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(store.available_nonce_count(GROUP, ME, 0).await.unwrap(), 4);
        assert!(store.take_nonce(GROUP, ME, 0, 1).await.unwrap().is_some());

        // Once taken, there is no nonce left to reveal, nor is an out-of-range
        // offset revealable.
        assert!(
            store
                .nonces_reveal(GROUP, ME, 0, 1)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .nonces_reveal(GROUP, ME, 0, 99)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn use_nonce_missing_tree_is_none() {
        let store = store().await;
        assert!(store.take_nonce(GROUP, ME, 7, 0).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn prune_group_nonces_removes_trees_and_nonces() {
        let store = store().await;
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(2))
            .await
            .unwrap()
            .unwrap();
        store.link_nonces_chunk(GROUP, ME, 0, root).await.unwrap();

        store.prune_group_nonces(GROUP).await.unwrap();
        // The cascade removed the nonces, so a use now no-ops.
        assert!(store.take_nonce(GROUP, ME, 0, 0).await.unwrap().is_none());
    }
}
