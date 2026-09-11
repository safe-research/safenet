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
//!   validator can still produce. They are retired once the keygen resolves.
//! - **Nonces** are handed out exactly once and are *removed* immediately when
//!   taken, in order to prevent accidental reuse. That deletion is permanent
//!   and nothing below ever restores it. Unused nonces persist so a re-included
//!   `preprocess` commitment can still be signed against, and are retired when
//!   the owning group is.
//!
//! Retiring a secret happens in two steps rather than at once:
//!
//! - **Scheduling.** A reconciliation effect carries the groups the state
//!   machine retained and the block it computed them for, and
//!   [`SecretStore::schedule_group_secrets_deletion`] records a deadline on
//!   every other group's rows. Reconciliations are ordered by that block, so a
//!   replayed or reordered one from an earlier block cannot undo a newer one's
//!   decision. A group that comes back has its deadline cancelled.
//! - **Collection.** The driver hands the effect handler its block status once
//!   per new block, and [`SecretStore::prune_scheduled_secrets`] deletes the
//!   rows due at or before the safe block - the same boundary state snapshots
//!   are pruned to, so a rollback the state machine can still make cannot
//!   outrun the deletion.
//!
//! Block numbers order reconciliations but do not identify chain branches, so a
//! deep rollback or a restart replaying past where a group was dropped can
//! still collect material a ceremony needs. Such a secret is treated like any
//! other that is missing rather than being recreated.

use crate::{
    bindings,
    frost::{
        keygen::Secrets,
        preprocess::{NonceChunk, Nonces},
    },
    metrics::{self, SecretKind},
};
use alloy::{
    hex::ToHexExt,
    primitives::{Address, B256},
};
use sqlx::{
    QueryBuilder, Sqlite,
    sqlite::{SqliteConnection, SqlitePool},
};
use std::{collections::BTreeSet, num::TryFromIntError};

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

/// The groups whose secrets a reconciliation keeps, by kind of secret.
///
/// Secrets of any other group are scheduled for deletion. The two sets differ
/// because a group can still need its persisted nonces after its DKG secrets
/// have served their purpose.
#[derive(Debug, Default)]
pub struct RetainedGroups {
    /// Groups whose DKG polynomial secrets are kept.
    pub keygen: BTreeSet<B256>,
    /// Groups whose nonce chunks, and the nonces under them, are kept.
    pub nonces: BTreeSet<B256>,
}

/// The secrets a collection removed, by kind of secret.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Pruned {
    /// DKG polynomial secret rows removed.
    pub keygen: u64,
    /// Nonce chunk rows removed, not counting the nonces cascaded with them.
    pub nonces: u64,
}

/// SQLite-backed store for locally-generated random secrets, over the shared
/// pool. Unlike the snapshot store, it is never rolled back on reorg.
pub struct SecretStore {
    pool: SqlitePool,
}

impl SecretStore {
    /// Creates the store backed by `pool`, creating its tables if absent.
    pub async fn new(pool: SqlitePool) -> Result<Self, Error> {
        // Both secret tables carry a nullable `delete_at_block`: the earliest
        // block at which the row may be deleted, `NULL` meaning no pending
        // deletion. `group_secret_reconciliation` holds the block of the last
        // accepted reconciliation as a single row, an empty table meaning none
        // has been accepted yet.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS keygen_secrets (
                 group_id        TEXT NOT NULL,
                 address         TEXT NOT NULL,
                 secrets         TEXT NOT NULL,
                 delete_at_block INTEGER,
                 PRIMARY KEY (group_id, address)
             );

             CREATE TABLE IF NOT EXISTS nonces_chunks (
                 root            TEXT NOT NULL,
                 group_id        TEXT NOT NULL,
                 address         TEXT NOT NULL,
                 delete_at_block INTEGER,
                 PRIMARY KEY (root)
             );

             CREATE TABLE IF NOT EXISTS nonces (
                 root  TEXT    NOT NULL,
                 offs  INTEGER NOT NULL,
                 nonce TEXT    NOT NULL,
                 PRIMARY KEY (root, offs),
                 FOREIGN KEY (root) REFERENCES nonces_chunks (root) ON DELETE CASCADE
             );

             CREATE TABLE IF NOT EXISTS group_secret_reconciliation (
                 id    INTEGER PRIMARY KEY CHECK (id = 0),
                 block INTEGER NOT NULL
             );

             CREATE INDEX IF NOT EXISTS idx_nonces_chunks_group
                 ON nonces_chunks (group_id);",
        )
        .execute(&pool)
        .await?;

        // Seed the secret gauges from the rows already on disk; every mutation
        // below keeps them in step from here on, so nothing else may add or
        // remove rows in these two tables.
        let (keygen, nonces) = sqlx::query_as::<_, (i64, i64)>(
            "SELECT (SELECT COUNT(*) FROM keygen_secrets),
                    (SELECT COUNT(*) FROM nonces_chunks)",
        )
        .fetch_one(&pool)
        .await?;
        metrics::secrets_total(SecretKind::Keygen).set(keygen as f64);
        metrics::secrets_total(SecretKind::Nonces).set(nonces as f64);

        Ok(Self { pool })
    }

    /// Persists the DKG `secrets` `me` generated for `group` and returns the
    /// secrets stored for that key.
    ///
    /// Existing secrets are **never overwritten**: a keygen commit effect
    /// reuses the retained secrets rather than resampling them, so a
    /// reorged-and-re-included commitment stays consistent with the shares the
    /// validator can still produce. Storing secrets for a group also cancels
    /// any deletion scheduled for them.
    pub async fn store_keygen_secrets(
        &self,
        group: B256,
        me: Address,
        secrets: Secrets,
    ) -> Result<Secrets, Error> {
        let mut tx = self.pool.begin().await?;

        // In case a keygen secret is already in the database, clear its
        // scheduled deletion (since it is requested for use).
        let existing = sqlx::query_scalar::<_, String>(
            "UPDATE keygen_secrets SET delete_at_block = NULL
             WHERE group_id = ? AND address = ?
             RETURNING secrets",
        )
        .bind(key(group))
        .bind(key(me))
        .fetch_optional(&mut *tx)
        .await?;

        let (stored, inserted) = if let Some(existing) = existing {
            (serde_json::from_str(&existing)?, 0)
        } else {
            sqlx::query("INSERT INTO keygen_secrets (group_id, address, secrets) VALUES (?, ?, ?)")
                .bind(key(group))
                .bind(key(me))
                .bind(serde_json::to_string(&secrets)?)
                .execute(&mut *tx)
                .await?;
            (secrets, 1)
        };

        tx.commit().await?;
        metrics::secrets_total(SecretKind::Keygen).increment(inserted);

        Ok(stored)
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
        metrics::secrets_total(SecretKind::Nonces).increment(1);

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

    /// Schedules the secrets of every group outside `retained` for deletion
    /// after `block`, and cancels the deletion scheduled for the groups in it.
    ///
    /// Reconciliations are ordered by the block they were computed for: a
    /// request below the last accepted one describes groups that have since
    /// moved on, so it is ignored and returns `false`. An equal or higher one
    /// is applied and returns `true` - equal blocks are accepted so a second
    /// reconciliation at the same height can still change what is retained.
    ///
    /// Nothing is deleted here: scheduled secrets stay readable and usable
    /// until they are collected. A group that is already scheduled keeps its
    /// original deletion block for as long as it stays absent, so reconciling
    /// repeatedly never pushes its deletion further out.
    ///
    /// Idempotent.
    pub async fn schedule_group_secrets_deletion(
        &self,
        block: u64,
        retained: &RetainedGroups,
    ) -> Result<bool, Error> {
        let block = i64::try_from(block)?;

        // The ordering decision is a conditional write inside the transaction
        // that applies it, so that concurrent reconciliations are ordered by
        // the database rather than by when they read the block. It returns no
        // row when the stored block is higher, leaving the schedules untouched.
        let mut tx = self.pool.begin().await?;
        let accepted = sqlx::query_scalar::<_, i64>(
            "INSERT INTO group_secret_reconciliation (id, block) VALUES (0, ?)
             ON CONFLICT (id) DO UPDATE
                 SET block = excluded.block
                 WHERE excluded.block >= group_secret_reconciliation.block
             RETURNING block",
        )
        .bind(block)
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
        if !accepted {
            return Ok(false);
        }

        // Both tables and the block marker move together: a failure here rolls
        // the marker back as well, so the same block can be reconciled again.
        schedule_absent_groups(&mut tx, "keygen_secrets", block, &retained.keygen).await?;
        schedule_absent_groups(&mut tx, "nonces_chunks", block, &retained.nonces).await?;
        tx.commit().await?;

        Ok(true)
    }

    /// Deletes the secrets whose deletion block `safe` has reached, and reports
    /// how many rows of each kind were removed.
    ///
    /// Deleting a nonce chunk cascades to the nonces under it, which the
    /// reported counts do not include. Secrets with no deletion block, and
    /// those whose block is still above `safe`, are left alone, as is the last
    /// accepted reconciliation block.
    ///
    /// Idempotent.
    pub async fn prune_scheduled_secrets(&self, safe: u64) -> Result<Pruned, Error> {
        let safe = i64::try_from(safe)?;

        let mut tx = self.pool.begin().await?;
        let keygen = sqlx::query("DELETE FROM keygen_secrets WHERE delete_at_block <= ?")
            .bind(safe)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        let nonces = sqlx::query("DELETE FROM nonces_chunks WHERE delete_at_block <= ?")
            .bind(safe)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        tx.commit().await?;
        metrics::secrets_total(SecretKind::Keygen).decrement(keygen as f64);
        metrics::secrets_total(SecretKind::Nonces).decrement(nonces as f64);

        Ok(Pruned { keygen, nonces })
    }
}

/// Schedules every row of `table` whose group is not in `retained` for deletion
/// after `block`, keeping the deletion block a row already has, and unschedules the
/// rows belonging to a retained group.
async fn schedule_absent_groups(
    connection: &mut SqliteConnection,
    table: &'static str,
    block: i64,
    retained: &BTreeSet<B256>,
) -> Result<(), Error> {
    let mut query = QueryBuilder::<Sqlite>::new(format!("UPDATE {table} SET delete_at_block = "));
    if retained.is_empty() {
        // Nothing is retained, so every row is absent:
        //
        //     UPDATE <table> SET delete_at_block = COALESCE(delete_at_block, <block>);
        query.push("COALESCE(delete_at_block, ");
        query.push_bind(block);
        query.push(")");
    } else {
        //     UPDATE <table>
        //        SET delete_at_block = CASE WHEN group_id IN (<retained>, ...) THEN NULL
        //                                   ELSE COALESCE(delete_at_block, <block>) END;
        query.push("CASE WHEN group_id IN (");
        let mut groups = query.separated(", ");
        for group in retained {
            groups.push_bind(key(*group));
        }
        groups.push_unseparated(") THEN NULL ELSE COALESCE(delete_at_block, ");
        groups.push_bind_unseparated(block);
        groups.push_unseparated(") END");
    }

    query.build().execute(connection).await?;
    Ok(())
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

    /// The block at which `group`'s DKG secrets are due for deletion, or `None`
    /// when none is scheduled.
    async fn keygen_delete_at_block(store: &SecretStore, group: B256) -> Option<i64> {
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT delete_at_block FROM keygen_secrets WHERE group_id = ? AND address = ?",
        )
        .bind(key(group))
        .bind(key(ME))
        .fetch_one(&store.pool)
        .await
        .unwrap()
    }

    /// The block at which the nonce chunk at `root` is due for deletion, or
    /// `None` when none is scheduled.
    async fn chunk_delete_at_block(store: &SecretStore, root: B256) -> Option<i64> {
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT delete_at_block FROM nonces_chunks WHERE root = ?",
        )
        .bind(key(root))
        .fetch_one(&store.pool)
        .await
        .unwrap()
    }

    /// The last accepted reconciliation block, or `None` while the marker is
    /// empty because no reconciliation has been accepted yet.
    async fn reconciliation_block(store: &SecretStore) -> Option<i64> {
        sqlx::query_scalar::<_, i64>("SELECT block FROM group_secret_reconciliation")
            .fetch_optional(&store.pool)
            .await
            .unwrap()
    }

    /// Retains `groups` for both kinds of secret.
    fn retained(groups: impl IntoIterator<Item = B256>) -> RetainedGroups {
        let groups = groups.into_iter().collect::<BTreeSet<_>>();
        RetainedGroups {
            keygen: groups.clone(),
            nonces: groups,
        }
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

        // A group being used again cancels the deletion scheduled for it,
        // still without resampling its secrets.
        assert!(
            store
                .schedule_group_secrets_deletion(1, &RetainedGroups::default())
                .await
                .unwrap()
        );
        assert_eq!(keygen_delete_at_block(&store, GROUP).await, Some(1));

        let stored = store
            .store_keygen_secrets(GROUP, ME, keygen_secrets())
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_string(&stored).unwrap(),
            serde_json::to_string(&first).unwrap(),
        );
        assert_eq!(keygen_delete_at_block(&store, GROUP).await, None);
    }

    #[tokio::test]
    async fn schedules_absent_groups_and_unschedules_retained_ones() {
        let store = store().await;
        let other = B256::repeat_byte(0xb2);
        for group in [GROUP, other] {
            store
                .store_keygen_secrets(group, ME, keygen_secrets())
                .await
                .unwrap();
        }

        // The first reconciliation is accepted whatever block it is for.
        assert!(
            store
                .schedule_group_secrets_deletion(0, &retained([GROUP]))
                .await
                .unwrap()
        );
        assert_eq!(keygen_delete_at_block(&store, GROUP).await, None);
        assert_eq!(keygen_delete_at_block(&store, other).await, Some(0));

        // A group that stays absent keeps the deletion block it was first given,
        // rather than having it pushed out on every reconciliation.
        assert!(
            store
                .schedule_group_secrets_deletion(7, &retained([GROUP]))
                .await
                .unwrap()
        );
        assert_eq!(keygen_delete_at_block(&store, other).await, Some(0));

        // Retaining it again cancels the deletion, and dropping it later
        // schedules it afresh.
        assert!(
            store
                .schedule_group_secrets_deletion(8, &retained([GROUP, other]))
                .await
                .unwrap()
        );
        assert_eq!(keygen_delete_at_block(&store, other).await, None);
        assert!(
            store
                .schedule_group_secrets_deletion(9, &retained([GROUP]))
                .await
                .unwrap()
        );
        assert_eq!(keygen_delete_at_block(&store, GROUP).await, None);
        assert_eq!(keygen_delete_at_block(&store, other).await, Some(9));
    }

    #[tokio::test]
    async fn each_kind_of_secret_follows_its_own_retained_set() {
        let store = store().await;
        store
            .store_keygen_secrets(GROUP, ME, keygen_secrets())
            .await
            .unwrap();
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(2))
            .await
            .unwrap();

        // A group whose DKG has resolved keeps the nonces it generated while
        // its DKG secrets are retired.
        assert!(
            store
                .schedule_group_secrets_deletion(
                    5,
                    &RetainedGroups {
                        keygen: BTreeSet::new(),
                        nonces: BTreeSet::from([GROUP]),
                    },
                )
                .await
                .unwrap()
        );

        assert_eq!(keygen_delete_at_block(&store, GROUP).await, Some(5));
        assert_eq!(chunk_delete_at_block(&store, root).await, None);
    }

    #[tokio::test]
    async fn reconciliation_below_the_last_accepted_block_is_ignored() {
        let store = store().await;
        store
            .store_keygen_secrets(GROUP, ME, keygen_secrets())
            .await
            .unwrap();
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(2))
            .await
            .unwrap();
        assert!(
            store
                .schedule_group_secrets_deletion(10, &retained([GROUP]))
                .await
                .unwrap()
        );

        // An outdated reconciliation schedules nothing and leaves the marker
        // where it is.
        assert!(
            !store
                .schedule_group_secrets_deletion(9, &RetainedGroups::default())
                .await
                .unwrap()
        );
        assert_eq!(keygen_delete_at_block(&store, GROUP).await, None);
        assert_eq!(chunk_delete_at_block(&store, root).await, None);
        assert_eq!(reconciliation_block(&store).await, Some(10));

        // One at the same height still applies, so a second reconciliation for
        // a block can change what it retains. Retaining nothing schedules
        // every secret in both tables.
        assert!(
            store
                .schedule_group_secrets_deletion(10, &RetainedGroups::default())
                .await
                .unwrap()
        );
        assert_eq!(keygen_delete_at_block(&store, GROUP).await, Some(10));
        assert_eq!(chunk_delete_at_block(&store, root).await, Some(10));
        assert_eq!(reconciliation_block(&store).await, Some(10));
    }

    #[tokio::test]
    async fn a_failed_reconciliation_rolls_back_the_marker_and_both_schedules() {
        let store = store().await;
        store
            .store_keygen_secrets(GROUP, ME, keygen_secrets())
            .await
            .unwrap();
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(2))
            .await
            .unwrap();
        assert!(
            store
                .schedule_group_secrets_deletion(3, &retained([GROUP]))
                .await
                .unwrap()
        );

        // Fail the second table's update, once the marker and the first table
        // have already been written within the transaction.
        sqlx::query(
            "CREATE TRIGGER fail_nonces_chunks BEFORE UPDATE ON nonces_chunks
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
        )
        .execute(&store.pool)
        .await
        .unwrap();
        assert!(
            store
                .schedule_group_secrets_deletion(4, &RetainedGroups::default())
                .await
                .is_err()
        );

        assert_eq!(reconciliation_block(&store).await, Some(3));
        assert_eq!(keygen_delete_at_block(&store, GROUP).await, None);
        assert_eq!(chunk_delete_at_block(&store, root).await, None);

        // Having changed nothing, the same block can be reconciled again.
        sqlx::query("DROP TRIGGER fail_nonces_chunks")
            .execute(&store.pool)
            .await
            .unwrap();
        assert!(
            store
                .schedule_group_secrets_deletion(4, &RetainedGroups::default())
                .await
                .unwrap()
        );
        assert_eq!(reconciliation_block(&store).await, Some(4));
        assert_eq!(keygen_delete_at_block(&store, GROUP).await, Some(4));
        assert_eq!(chunk_delete_at_block(&store, root).await, Some(4));
    }

    #[tokio::test]
    async fn collects_only_secrets_scheduled_at_or_before_the_safe_block() {
        let store = store().await;
        let other = B256::repeat_byte(0xb2);
        for group in [GROUP, other] {
            store
                .store_keygen_secrets(group, ME, keygen_secrets())
                .await
                .unwrap();
        }
        let scheduled = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(2))
            .await
            .unwrap();
        assert!(
            store
                .schedule_group_secrets_deletion(5, &retained([other]))
                .await
                .unwrap()
        );
        // Registered after the reconciliation, so nothing is scheduled for it.
        let unscheduled = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(2))
            .await
            .unwrap();

        // A deletion block past the safe block is not due yet.
        assert_eq!(
            store.prune_scheduled_secrets(4).await.unwrap(),
            Pruned::default()
        );
        assert!(get_keygen_secrets(&store, GROUP).await.is_some());

        // The deletion block itself is due, and unscheduled secrets are left alone.
        assert_eq!(
            store.prune_scheduled_secrets(5).await.unwrap(),
            Pruned {
                keygen: 1,
                nonces: 1,
            }
        );
        assert!(get_keygen_secrets(&store, GROUP).await.is_none());
        assert!(get_keygen_secrets(&store, other).await.is_some());
        assert_eq!(count_root_nonces(&store, scheduled).await, 0);
        assert_eq!(count_root_nonces(&store, unscheduled).await, 2);

        // Collection is repeatable and leaves the ordering marker alone, so a
        // replayed reconciliation below it stays ignored.
        assert_eq!(
            store.prune_scheduled_secrets(5).await.unwrap(),
            Pruned::default()
        );
        assert_eq!(reconciliation_block(&store).await, Some(5));
    }

    #[tokio::test]
    async fn a_failed_collection_deletes_nothing() {
        let store = store().await;
        store
            .store_keygen_secrets(GROUP, ME, keygen_secrets())
            .await
            .unwrap();
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(2))
            .await
            .unwrap();
        assert!(
            store
                .schedule_group_secrets_deletion(1, &RetainedGroups::default())
                .await
                .unwrap()
        );

        // Fail the chunk deletion, once the DKG secrets have already been
        // deleted within the transaction.
        sqlx::query(
            "CREATE TRIGGER fail_nonces_chunks BEFORE DELETE ON nonces_chunks
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END",
        )
        .execute(&store.pool)
        .await
        .unwrap();
        assert!(store.prune_scheduled_secrets(1).await.is_err());

        assert!(get_keygen_secrets(&store, GROUP).await.is_some());
        assert_eq!(count_root_nonces(&store, root).await, 2);

        // The collection is retried by the next block status, still scheduled.
        sqlx::query("DROP TRIGGER fail_nonces_chunks")
            .execute(&store.pool)
            .await
            .unwrap();
        assert_eq!(
            store.prune_scheduled_secrets(1).await.unwrap(),
            Pruned {
                keygen: 1,
                nonces: 1,
            }
        );
        assert_eq!(count_root_nonces(&store, root).await, 0);
    }

    #[tokio::test]
    async fn scheduled_nonces_stay_revealable_and_consumable() {
        let store = store().await;
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(2))
            .await
            .unwrap();
        assert!(
            store
                .schedule_group_secrets_deletion(1, &RetainedGroups::default())
                .await
                .unwrap()
        );

        // Scheduling is not deletion: the nonces are usable until collected.
        assert_eq!(chunk_delete_at_block(&store, root).await, Some(1));
        assert!(store.nonces_reveal(root, 0).await.unwrap().is_some());
        assert!(store.take_nonce(root, 0).await.unwrap().is_some());
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

    #[tokio::test]
    async fn reopening_preserves_stored_secrets() {
        let store = store().await;
        store
            .store_keygen_secrets(GROUP, ME, keygen_secrets())
            .await
            .unwrap();
        let root = store
            .register_nonces_chunk(GROUP, ME, nonce_chunk(3))
            .await
            .unwrap();
        assert!(store.take_nonce(root, 0).await.unwrap().is_some());

        // Creating the store again re-runs the schema setup exactly as a
        // restart does, so the added table and columns must leave the secrets
        // already in the database untouched.
        let store = SecretStore::new(store.pool.clone()).await.unwrap();

        assert!(get_keygen_secrets(&store, GROUP).await.is_some());
        assert_eq!(count_root_nonces(&store, root).await, 2);
        // A consumed nonce is never restored.
        assert!(store.nonces_reveal(root, 0).await.unwrap().is_none());
    }
}
