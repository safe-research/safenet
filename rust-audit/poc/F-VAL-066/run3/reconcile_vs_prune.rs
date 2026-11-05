//! RUN 3 (rv-val, 2026-10-08): F-VAL-066 / F2-VAL-035 at `361aaf3`. Child
//! module of `crate::service::effect`. Adapted from
//! `poc/F2-XC-050/coverage-7.3/qa2_cov_7_3_effect_test.rs` (run 2): the
//! retained set is now `BTreeMap<B256, RetainedSecrets>` and a DKG group is
//! retained as `RetainedSecrets::All`. Needs `RV_VAL_DB_DIR` (a scratch dir).
//!
//! The driver still spawns a block's `ReconcileGroupSecrets` and awaits the
//! prune inline in the same input whenever that `New` update applies the
//! block's transition, which is the case for the first `New` after an `Uncle`
//! (`crates/core/src/state/mod.rs`, `Status::BlockPending`), i.e. after every
//! reorg and every restart. Test 1 repeats run 2's race in that construction.
//! Test 2 is the short-outage geometry: every replayed reconciliation below
//! the previous run's marker is ignored, so the first prune runs before any
//! reconciliation can cancel a stale schedule.

use super::*;
use crate::frost::keygen;
use alloy::primitives::address;
use safenet_core::{effects::EffectManager, index::BlockStatus, utils::connect_sqlite};
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;

const GROUP: B256 = B256::repeat_byte(0x73);
const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");

fn fresh_db(name: &str) -> String {
    let dir = std::env::var("RV_VAL_DB_DIR").expect("RV_VAL_DB_DIR must name a scratch directory");
    let path = format!("{dir}/{name}.db");
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{path}{suffix}"));
    }
    path
}

/// A file-backed store holding GROUP's DKG secret, scheduled for deletion at
/// `scheduled` by the previous run, whose last accepted reconciliation was at
/// `marker`.
async fn restored_store(name: &str, scheduled: u64, marker: u64) -> SqlitePool {
    let path = fresh_db(name);
    let options = SqliteConnectOptions::from_str(&format!("sqlite:{path}?mode=rwc")).unwrap();
    let pool = connect_sqlite(options).await.unwrap();
    let store = SecretStore::new(pool.clone()).await.unwrap();
    let secrets = keygen::setup(&mut rand::thread_rng(), ME, 3, 2).unwrap();
    store.store_keygen_secrets(GROUP, ME, secrets).await.unwrap();
    assert!(store.schedule_group_secrets_deletion(scheduled, &BTreeMap::new()).await.unwrap());
    if marker > scheduled {
        // Later reconciliations of the old run, which also did not retain GROUP.
        assert!(store.schedule_group_secrets_deletion(marker, &BTreeMap::new()).await.unwrap());
    }
    assert_eq!(delete_at_block(&pool).await, Some(Some(scheduled as i64)));
    pool
}

async fn delete_at_block(pool: &SqlitePool) -> Option<Option<i64>> {
    sqlx::query_scalar::<_, Option<i64>>("SELECT delete_at_block FROM keygen_secrets")
        .fetch_optional(pool)
        .await
        .unwrap()
}

fn reconcile(block: u64) -> Effect {
    Effect::ReconcileGroupSecrets {
        block,
        groups: BTreeMap::from([(GROUP, RetainedSecrets::All)]),
    }
}

/// Run 2's construction: spawn the reconciliation, then the inline prune, for
/// the first live block `{ latest: 110, safe: 105 }` with the old run's
/// schedule at 100.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_input_spawn_then_prune_race() {
    const ROUNDS: usize = 20;
    let live = BlockStatus { latest: 110, safe: 105 };
    let mut lost = 0;
    for round in 0..ROUNDS {
        let pool = restored_store(&format!("rv-val-066-race-{round}"), 100, 100).await;
        let mut effects =
            EffectManager::new(Handler::new(ME, SecretStore::new(pool.clone()).await.unwrap()));
        effects.spawn(reconcile(live.safe));
        effects.housekeeping(live).await;
        let _ = effects.next().await;
        let row = delete_at_block(&pool).await;
        if row.is_none() {
            lost += 1;
        }
        println!("round {round:>2}: keygen_secrets row = {row:?}");
        pool.close().await;
    }
    println!("same-input spawn then prune, {ROUNDS} rounds on 2 workers: secret lost in {lost}, retained in {}", ROUNDS - lost);
}

/// Short outage, `max_reorg_depth = 5`: the old run reconciled up to block
/// 1000 and had scheduled GROUP at 997 (its branch dropped the group); a
/// depth-5 reorg during a 3-block outage undid the drop. On restart at head
/// 1003 the replay reconciles 996..999 (all below the marker, ignored) and
/// the first `New { 999 }` prunes with `safe = 998` before the first accepted
/// reconciliation (block 1000) exists.
#[tokio::test]
async fn short_outage_prune_precedes_every_accepted_reconciliation() {
    let pool = restored_store("rv-val-066-short-outage", 997, 1000).await;
    let handler = Handler::new(ME, SecretStore::new(pool.clone()).await.unwrap());
    for block in 996..=999 {
        let resume = handler.perform_effect(reconcile(block)).await;
        assert!(matches!(resume, Resume::Noop));
    }
    assert_eq!(delete_at_block(&pool).await, Some(Some(997)), "below-marker reconciliations ignored");
    handler.housekeeping(BlockStatus { latest: 1003, safe: 998 }).await;
    let after_prune = delete_at_block(&pool).await;
    let _ = handler.perform_effect(reconcile(1000)).await;
    let after_reconcile = delete_at_block(&pool).await;
    println!("short outage: after New(999) prune row = {after_prune:?}; after the first accepted reconciliation (1000) row = {after_reconcile:?}");
    assert_eq!(after_reconcile, None, "the retained group's DKG secret is gone for good");
}
