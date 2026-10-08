
    #[tokio::test]
    async fn f2_val_034_old_schema_db_opens_then_fails_on_the_new_column() {
        // QA2-VAL-B PoC for F2-VAL-034 (temporary; reverted after the run).
        // A database created with the pre-pruning schema (commit 49d7e39: no
        // `delete_at_block` column, no `group_secret_reconciliation` table) is
        // opened by the current `SecretStore::new` without error, then fails at
        // first use.
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE keygen_secrets (
                 group_id TEXT NOT NULL,
                 address  TEXT NOT NULL,
                 secrets  TEXT NOT NULL,
                 PRIMARY KEY (group_id, address)
             );
             CREATE TABLE nonces_chunks (
                 root     TEXT NOT NULL,
                 group_id TEXT NOT NULL,
                 address  TEXT NOT NULL,
                 PRIMARY KEY (root)
             );
             CREATE TABLE nonces (
                 root  TEXT    NOT NULL,
                 offs  INTEGER NOT NULL,
                 nonce TEXT    NOT NULL,
                 PRIMARY KEY (root, offs),
                 FOREIGN KEY (root) REFERENCES nonces_chunks (root) ON DELETE CASCADE
             );",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Opening succeeds: CREATE TABLE IF NOT EXISTS does NOT add the missing
        // column to the pre-existing tables. (It DOES create the absent
        // `group_secret_reconciliation` table, so the finding's "no such table"
        // wording is imprecise -- the real failure is the missing COLUMN.)
        let store = SecretStore::new(pool.clone()).await.expect("old DB opens without error");

        // First use of `store_keygen_secrets` (KeyGenSetup) fails: it UPDATEs
        // `delete_at_block`, which the old table lacks.
        let err = store
            .store_keygen_secrets(GROUP, ME, keygen_secrets())
            .await
            .expect_err("must fail on the missing column");
        eprintln!("F2-VAL-034 store_keygen_secrets error: {err}");
        assert!(format!("{err}").contains("delete_at_block"), "error names the missing column");

        // Reconciliation also fails on the same column (inside schedule_absent_
        // groups' UPDATE), so no nonce stream is ever started (feeds F2-VAL-031).
        let err = store
            .schedule_group_secrets_deletion(100, &retained([GROUP]))
            .await
            .expect_err("reconciliation must fail on the missing column");
        eprintln!("F2-VAL-034 schedule_group_secrets_deletion error: {err}");
        assert!(format!("{err}").contains("delete_at_block"));
    }
