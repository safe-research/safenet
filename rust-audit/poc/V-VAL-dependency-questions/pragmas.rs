//! V-VAL Phase 5 — settles shared question 12 and VAL-Q3/VAL-Q4 by execution.
//!
//! Wire in via `crates/validator/src/secrets/mod.rs`:
//! ```ignore
//! #[cfg(test)]
//! #[path = "../../../../rust-audit/poc/V-VAL-dependency-questions/pragmas.rs"]
//! mod poc_v_val_pragmas;
//! ```
//! Run: cargo test -p validator --bins secrets::poc_v_val_pragmas -- --nocapture

use sqlx::Row;
use std::time::{SystemTime, UNIX_EPOCH};

async fn report(label: &str, pool: &sqlx::SqlitePool) {
    println!("\n=== {label} ===");
    for pragma in [
        "foreign_keys",
        "journal_mode",
        "synchronous",
        "busy_timeout",
        "page_size",
        "locking_mode",
    ] {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA {pragma}")))
            .fetch_optional(pool)
            .await
            .unwrap_or_else(|e| panic!("PRAGMA {pragma}: {e}"));
        let value = match row {
            None => "<no row>".to_string(),
            Some(row) => row
                .try_get::<i64, _>(0)
                .map(|v| v.to_string())
                .or_else(|_| row.try_get::<String, _>(0))
                .unwrap_or_else(|e| format!("<undecodable: {e}>")),
        };
        println!("{pragma} = {value}");
    }
    let max = pool.options().get_max_connections();
    let min = pool.options().get_min_connections();
    println!("pool max_connections = {max}, min_connections = {min}");
}

/// Question 12 / VAL-Q3 / VAL-Q4: what does `connect_sqlite` actually give us?
#[tokio::test]
async fn sqlite_defaults_under_connect_sqlite() {
    // Exactly the shape `crates/validator/src/main.rs` uses: options parsed
    // from the config's database URL, then `connect_sqlite`.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("poc-v-val-pragmas-{nanos}.db"));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let options: sqlx::sqlite::SqliteConnectOptions = url.parse().expect("parse database url");
    let pool = safenet_core::utils::connect_sqlite(options)
        .await
        .expect("connect");
    report("file-backed, url-parsed, exactly as main.rs", &pool).await;

    // The decisive one for F-VAL-035 (c).
    let fk: i64 = sqlx::query("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        fk, 1,
        "VAL-Q3 / question 12: foreign_keys is OFF, so ON DELETE CASCADE is decorative"
    );

    pool.close().await;
    let _ = std::fs::remove_file(&path);
}

/// The cascade itself, end to end on the shipped schema — the concrete form of
/// VAL-Q3's proposed `retain_nonces_cascades`.
#[tokio::test]
async fn on_delete_cascade_actually_fires() {
    use crate::frost::{keygen::KeyShare, preprocess::NonceChunk};
    use alloy::primitives::{Address, B256, address};

    const GROUP: B256 = B256::repeat_byte(0xa1);
    const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

    let options: sqlx::sqlite::SqliteConnectOptions =
        "sqlite::memory:".parse().expect("parse url");
    let pool = safenet_core::utils::connect_sqlite(options).await.unwrap();
    let store = crate::secrets::SecretStore::new(pool.clone()).await.unwrap();

    let chunk = NonceChunk::with_size(4, &KeyShare::dummy(), &mut rand::thread_rng()).unwrap();
    let _root = store.register_nonces_chunk(GROUP, ME, chunk).await.unwrap();

    let before: i64 = sqlx::query("SELECT COUNT(*) FROM nonces")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    println!("nonces rows before retain_nonces([]) = {before}");
    assert_eq!(before, 4);

    store.retain_nonces([]).await.unwrap();

    let after: i64 = sqlx::query("SELECT COUNT(*) FROM nonces")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    let chunks: i64 = sqlx::query("SELECT COUNT(*) FROM nonces_chunks")
        .fetch_one(&pool)
        .await
        .unwrap()
        .get(0);
    println!("nonces rows after  retain_nonces([]) = {after}, chunk rows = {chunks}");
    assert_eq!(chunks, 0, "the chunk row is deleted by retain_nonces");
    assert_eq!(
        after, 0,
        "F-VAL-035(c): the child rows were ORPHANED, not cascaded — foreign keys are not enforced"
    );

    pool.close().await;
}
