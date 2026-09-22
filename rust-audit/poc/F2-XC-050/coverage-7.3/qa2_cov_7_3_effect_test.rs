// ======================================================================
// QA2-XC (run 2): coverage.md section 7 item 3 -- housekeeping vs the same
// block's reconciliation on a restart (R5 O1). Temporarily appended to the
// end of `crates/validator/src/service/effect.rs` (a new `#[cfg(test)]`
// module, the file has none of its own) and reverted with
// `git checkout -- crates/validator/src/service/effect.rs` after the run.
//
// What the driver does for the first live `New { n }` after a catch-up warp
// (`crates/core/src/driver.rs:257-263, 272-294`): it applies the block (the
// validator's block advance returns `Effect::ReconcileGroupSecrets { block,
// groups }`, `state/preprocess.rs:168-173`), **spawns** that effect onto a
// tokio task (`effects.spawn`, `core/src/effects.rs:61-69`) and then runs
// `housekeeping(status)` **inline** on the driver task (`driver.rs:292-294`),
// which is `SecretStore::prune_scheduled_secrets(status.safe)`
// (`service/effect.rs:278-283`). Nothing orders the two: the reconciliation
// that would cancel a stale schedule races the prune that collects it.
//
// The precondition injected here is R5 O1's: the previous run's last accepted
// reconciliation (block 100) scheduled GROUP's DKG secrets for deletion
// because *that branch* had dropped the group, while the branch replayed
// after the outage still needs it (reorg inside `max_reorg_depth` during the
// downtime), so the replayed reconciliation at block 105 retains GROUP.
// Which of the two SQLite writes lands first decides whether the secret
// survives. Test 1 pins both deterministic orders; test 2 runs the driver's
// real construction (`EffectManager::spawn` + `housekeeping`) on a
// multi-threaded runtime, as `#[tokio::main]` gives the binary, and counts.
// ======================================================================
#[cfg(test)]
mod qa2_cov_7_3 {
    use super::*;
    use crate::{frost::keygen, secrets::store::RetainedGroups};
    use alloy::primitives::address;
    use safenet_core::{effects::EffectManager, index::BlockStatus, utils::connect_sqlite};
    use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
    use std::{collections::BTreeMap, str::FromStr};

    const GROUP: B256 = B256::repeat_byte(0x73);
    const ME: Address = address!("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
    /// The previous run's last accepted reconciliation (schedules GROUP).
    const OLD_BRANCH_BLOCK: u64 = 100;
    /// The first live block after the restart's warp, and the watcher's
    /// `safe` for it (`max_reorg_depth = 5`).
    const LIVE: BlockStatus = BlockStatus {
        latest: 110,
        safe: 105,
    };

    fn fresh_db(name: &str) -> String {
        let dir = std::env::var("QA2_XC_DIR").expect("QA2_XC_DIR must name a scratch directory");
        let path = format!("{dir}/{name}.db");
        for suffix in ["", "-journal", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{path}{suffix}"));
        }
        path
    }

    /// A file-backed pool opened as the validator opens it
    /// (`utils::connect_sqlite`, no pragmas), holding one DKG secret for
    /// GROUP that the old branch scheduled for deletion at block 100.
    async fn restored_store(name: &str) -> (SqlitePool, SecretStore) {
        let path = fresh_db(name);
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{path}?mode=rwc")).unwrap();
        let pool = connect_sqlite(options).await.unwrap();
        let store = SecretStore::new(pool.clone()).await.unwrap();
        let secrets = keygen::setup(&mut rand::thread_rng(), ME, 3, 2).unwrap();
        store.store_keygen_secrets(GROUP, ME, secrets).await.unwrap();
        // The old branch dropped GROUP at block 100: scheduled, not yet collected.
        assert!(
            store
                .schedule_group_secrets_deletion(OLD_BRANCH_BLOCK, &RetainedGroups::default())
                .await
                .unwrap()
        );
        assert_eq!(delete_at_block(&pool).await, Some(Some(OLD_BRANCH_BLOCK as i64)));
        (pool, store)
    }

    /// `Some(delete_at_block)` while GROUP's row exists, `None` once pruned.
    async fn delete_at_block(pool: &SqlitePool) -> Option<Option<i64>> {
        sqlx::query_scalar::<_, Option<i64>>("SELECT delete_at_block FROM keygen_secrets")
            .fetch_optional(pool)
            .await
            .unwrap()
    }

    /// The replayed branch's reconciliation for the first live block: GROUP
    /// is still in DKG (retained without a key share).
    fn replayed_reconciliation() -> Effect {
        Effect::ReconcileGroupSecrets {
            block: LIVE.safe,
            groups: BTreeMap::from([(GROUP, None)]),
        }
    }

    #[tokio::test]
    async fn the_secret_survives_or_is_lost_depending_only_on_the_order_of_the_two_writes() {
        // Order A: the reconciliation lands first (its `block` 105 >= 100 is
        // accepted, store.rs:311-345) and cancels the schedule; the prune
        // then finds nothing due.
        let (pool, store) = restored_store("cov73-order-a").await;
        let handler = Handler::new(ME, SecretStore::new(pool.clone()).await.unwrap());
        let resume = handler.perform_effect(replayed_reconciliation()).await;
        assert!(matches!(resume, Resume::Noop));
        handler.housekeeping(LIVE).await;
        let a = delete_at_block(&pool).await;
        println!("order A (reconcile, then housekeeping): keygen_secrets row for GROUP = {a:?}");
        assert_eq!(a, Some(None), "cancelled schedule, secret retained");
        drop(store);
        pool.close().await;

        // Order B: the inline prune runs first (`delete_at_block 100 <= safe
        // 105`), then the reconciliation retains a group that no longer has a
        // row -- `schedule_absent_groups` only touches existing rows.
        let (pool, store) = restored_store("cov73-order-b").await;
        let handler = Handler::new(ME, SecretStore::new(pool.clone()).await.unwrap());
        handler.housekeeping(LIVE).await;
        let resume = handler.perform_effect(replayed_reconciliation()).await;
        assert!(matches!(resume, Resume::Noop));
        let b = delete_at_block(&pool).await;
        println!("order B (housekeeping, then reconcile): keygen_secrets row for GROUP = {b:?}");
        assert_eq!(b, None, "the DKG secret the replayed branch still needs is gone");
        drop(store);
        pool.close().await;
    }

    /// The driver's own construction: `effects.spawn(effect)` (driver.rs:278)
    /// immediately followed by `effects.housekeeping(status).await`
    /// (driver.rs:293), on the multi-threaded runtime `#[tokio::main]`
    /// gives both binaries. Repeated so the observed order is a sample, not
    /// a single roll; nothing is asserted about the race itself.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_drivers_spawn_then_housekeeping_order_leaves_the_outcome_to_the_scheduler() {
        const ROUNDS: usize = 20;
        let mut lost = 0;
        for round in 0..ROUNDS {
            let (pool, store) = restored_store(&format!("cov73-race-{round}")).await;
            let mut effects =
                EffectManager::new(Handler::new(ME, SecretStore::new(pool.clone()).await.unwrap()));
            // driver.rs:278
            effects.spawn(replayed_reconciliation());
            // driver.rs:292-294, inline on the driver task
            effects.housekeeping(LIVE).await;
            // the effect task finishes (Resume::Noop) before the next input
            let _ = effects.next().await;
            let row = delete_at_block(&pool).await;
            let outcome = match row {
                None => {
                    lost += 1;
                    "LOST (prune won)"
                }
                Some(None) => "retained (reconcile won)",
                Some(Some(block)) => panic!("unexpected schedule {block} left behind"),
            };
            println!("round {round:>2}: keygen_secrets row = {row:?} -> {outcome}");
            drop(store);
            pool.close().await;
        }
        println!(
            "driver order (spawn reconcile, then inline housekeeping), {ROUNDS} rounds on a 2-worker runtime: \
             secret lost in {lost}, retained in {}",
            ROUNDS - lost
        );
    }
}
