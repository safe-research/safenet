//! PoC part 1 for F-VAL-033 — restoring the validator database un-burns a
//! consumed FROST signing nonce.
//!
//! NEVER COMPILED. See `README.md` in this directory.
//!
//! Include as a child of `crate::secrets` by adding to
//! `crates/validator/src/secrets/mod.rs`:
//!
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/F-VAL-033/store_restore.rs"]
//! mod poc_f_val_033;
//! ```
//!
//! This half proves the *store* half of the finding with a literal file-level
//! backup and restore — no private field access, no mocking. The signing half
//! (two messages, one nonce) is in `nonce_reuse.rs`, which is included under
//! `crate::frost` instead.

use super::store::SecretStore;
use crate::frost::{keygen::KeyShare, preprocess::NonceChunk};
use alloy::primitives::{Address, B256, address};
use sqlx::sqlite::SqlitePool;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const GROUP: B256 = B256::repeat_byte(0xa1);
const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

/// The offset the reused nonce sits at.
///
/// Chosen to model contract-assigned sequence `s = 1030`:
/// `decode_sequence(1030) = (chunk 1, offset 6)`
/// (`crates/validator/src/frost/preprocess.rs:28-33`, `SEQUENCE_CHUNK_SIZE = 1024`).
/// The offset is all `take_nonce` sees.
const OFFSET: u64 = 6;

/// A scratch directory that is removed on drop, standing in for the operator's
/// data directory. `tempfile` is not a dependency of this crate, so this is
/// hand-rolled.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("poc-f-val-033-{tag}-{nanos}"));
        fs::create_dir_all(&path).expect("scratch dir");
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Opens the validator's SQLite file exactly as `main.rs:46` does, via
/// `safenet_core::utils::connect_sqlite`, and wraps it in a `SecretStore`.
async fn open(path: &Path) -> (SqlitePool, SecretStore) {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = safenet_core::utils::connect_sqlite(options)
        .await
        .expect("open sqlite");
    let store = SecretStore::new(pool.clone()).await.expect("store");
    (pool, store)
}

/// Copies the database file and any sidecars, i.e. what an operator's backup
/// script does. `pool.close()` must have been awaited first so SQLite has
/// checkpointed and removed the WAL.
fn copy_db(from: &Path, to: &Path) {
    fs::copy(from, to).expect("copy db");
    for suffix in ["-wal", "-shm"] {
        let src = PathBuf::from(format!("{}{suffix}", from.display()));
        if src.exists() {
            let _ = fs::copy(&src, PathBuf::from(format!("{}{suffix}", to.display())));
        }
    }
}

/// **The finding, at the store layer.**
///
/// PASS ⇒ `take_nonce` hands out the *same* secret nonce a second time after a
/// file-level restore. The store's module doc claim that nonces are "handed out
/// exactly once" (`crates/validator/src/secrets/store.rs:19-20`) and
/// `take_nonce`'s claim that "deletion is permanent" (`:200-204`) are true only
/// of the current file, not of the validator's history. F-VAL-033's core
/// mechanism is `E1`-confirmed.
///
/// FAIL ⇒ something outside `store.rs` records consumption durably, or the
/// restore path is blocked; F-VAL-033 is refuted at the store layer and the
/// finding should be closed.
///
/// After remediation option 1 or 3 this test **must fail** at the marked
/// assertion. Keep it as an inverted regression test.
#[tokio::test]
async fn restore_unburns_a_consumed_nonce() {
    let scratch = Scratch::new("restore");
    let live = scratch.join("validator.db");
    let backup = scratch.join("validator.db.bak");

    // --- t0: the validator is running. A preprocess chunk is registered. ---
    let (pool, store) = open(&live).await;
    let chunk = NonceChunk::with_size(16, &KeyShare::dummy(), &mut rand::thread_rng())
        .expect("chunk generation");
    let root = store
        .register_nonces_chunk(GROUP, ME, chunk)
        .await
        .expect("register");
    let (revealed_before, _proof) = store
        .nonces_reveal(root, OFFSET)
        .await
        .expect("reveal")
        .expect("the nonce exists before it is burned");
    pool.close().await;

    // --- t1: the operator takes the backup the handbook instructs them to
    // ---     take (`docs/validator-handbook.md:19` and `:75`). ---
    copy_db(&live, &backup);

    // --- t2: the chain assigns sequence 1030 to message `m`. The validator
    // ---     reveals and then burns the nonce, publishing `z(m)`. ---
    let (pool, store) = open(&live).await;
    let burned = store
        .take_nonce(root, OFFSET)
        .await
        .expect("take")
        .expect("the nonce is available the first time");
    let (burned_commitments, _) = burned.reveal();
    assert_eq!(
        burned_commitments, revealed_before,
        "the burned nonce is the one that was revealed onchain"
    );
    // The store now behaves exactly as documented: a replay no-ops.
    assert!(
        store.take_nonce(root, OFFSET).await.expect("take").is_none(),
        "within one database file, the nonce really is single-use"
    );
    pool.close().await;

    // --- t3: a reorg no deeper than `max_reorg_depth` rebinds sequence 1030 to
    // ---     a different message `m'` (A5). The disk fails, or a bad deploy is
    // ---     rolled back, and the operator restores the t1 backup. ---
    fs::remove_file(&live).expect("simulate disk loss");
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(PathBuf::from(format!("{}{suffix}", live.display())));
    }
    copy_db(&backup, &live);

    // --- t4: the validator replays. `observe` maps sequence 1030 to the same
    // ---     `(root, offset)` (`state/preprocess.rs:180-193`), the contract
    // ---     accepts the reveal because the offset still matches the sequence
    // ---     (`FROSTNonceCommitmentSet.sol:116-134`), and the store hands the
    // ---     secret out again. ---
    let (pool, store) = open(&live).await;
    let reused = store
        .take_nonce(root, OFFSET)
        .await
        .expect("take")
        // *** THE DEFECT ***
        .expect(
            "F-VAL-033: the restored database handed out an already-consumed nonce. \
             If this `expect` does NOT fire, the finding is confirmed.",
        );

    let (reused_commitments, _) = reused.reveal();
    assert_eq!(
        reused_commitments, burned_commitments,
        "the same (d, e) commitment pair is available for a second, different message"
    );
    pool.close().await;
}

/// Control: a restore with **no intervening consumption** is harmless, and a
/// restore *forward* of the burn is harmless too. This is the reviewer's own
/// "considered and rejected" entry, made executable — it matters because a fix
/// must not break same-chain replay idempotence.
///
/// PASS ⇒ the hazard is specifically the restore of a *stale* file, which is
/// what remediation option 3 targets.
#[tokio::test]
async fn restoring_a_newer_backup_is_harmless() {
    let scratch = Scratch::new("newer");
    let live = scratch.join("validator.db");
    let backup = scratch.join("validator.db.bak");

    let (pool, store) = open(&live).await;
    let chunk = NonceChunk::with_size(16, &KeyShare::dummy(), &mut rand::thread_rng()).unwrap();
    let root = store.register_nonces_chunk(GROUP, ME, chunk).await.unwrap();
    assert!(store.take_nonce(root, OFFSET).await.unwrap().is_some());
    pool.close().await;

    // Backup taken *after* the burn.
    copy_db(&live, &backup);
    fs::remove_file(&live).unwrap();
    copy_db(&backup, &live);

    let (pool, store) = open(&live).await;
    assert!(
        store.take_nonce(root, OFFSET).await.unwrap().is_none(),
        "a backup taken after the burn stays burned"
    );
    pool.close().await;
}
