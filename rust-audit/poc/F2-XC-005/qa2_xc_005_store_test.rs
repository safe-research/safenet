    // ======================================================================
    // QA2-XC proof-of-concept modules (run 2). Temporarily pasted at the end
    // of `crates/validator/src/secrets/store.rs`'s `mod tests` block; reverted
    // with `git checkout -- crates/validator/src/secrets/store.rs` after the run.
    //
    //   qa2_xc_005    F2-XC-005: `prune_scheduled_secrets` is a logical DELETE;
    //                 whether the secret's bytes leave the database file depends
    //                 on SQLite's `secure_delete` (a libsqlite3 build flag the
    //                 project does not pin), and the rollback journal receives
    //                 the page pre-image either way.
    //   qa2_cov_7_6   coverage.md section 7 item 6: an out-of-range `Operation`
    //                 in a `TransactionProposed` log decodes to `__Invalid`,
    //                 not to a decode error (refinement for F2-VAL-061 /
    //                 F2-CORE-004). Source saved under poc/F2-CORE-004/enum-decode/.
    // ======================================================================
    mod qa2_xc_005 {
        use super::*;
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;

        fn fresh_db(name: &str) -> String {
            let dir = std::env::var("QA2_XC_DIR").expect("QA2_XC_DIR must name a scratch directory");
            let path = format!("{dir}/{name}.db");
            for suffix in ["", "-journal", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{path}{suffix}"));
            }
            path
        }

        /// The store over a file-backed pool opened exactly as the validator
        /// opens it (`safenet_core::utils::connect_sqlite`, no pragmas), with an
        /// optional `secure_delete` override standing in for a libsqlite3 built
        /// with or without `SQLITE_SECURE_DELETE`.
        async fn file_store(path: &str, secure_delete: Option<&str>) -> (SecretStore, SqlitePool) {
            let mut options = SqliteConnectOptions::from_str(&format!("sqlite:{path}?mode=rwc")).unwrap();
            if let Some(value) = secure_delete {
                options = options.pragma("secure_delete", value.to_owned());
            }
            let pool = safenet_core::utils::connect_sqlite(options).await.unwrap();
            let store = SecretStore::new(pool.clone()).await.unwrap();
            (store, pool)
        }

        /// The first run of at least 64 hex digits in the stored JSON: a scalar
        /// of the DKG secret, short enough to sit inside one database page.
        fn secret_marker(json: &str) -> &str {
            let bytes = json.as_bytes();
            let mut start = 0;
            while start < bytes.len() {
                let len = bytes[start..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
                if len >= 64 {
                    return &json[start..start + 64];
                }
                start += len + 1;
            }
            panic!("no 64-hex-digit run in the stored secret");
        }

        fn contains(haystack: &[u8], needle: &[u8]) -> bool {
            haystack.windows(needle.len()).any(|window| window == needle)
        }

        async fn stored_json(pool: &SqlitePool) -> String {
            sqlx::query_scalar::<_, String>("SELECT secrets FROM keygen_secrets WHERE group_id = ?")
                .bind(key(GROUP))
                .fetch_one(pool)
                .await
                .unwrap()
        }

        /// Stores a DKG secret, schedules and collects it through the real store
        /// API, and reports whether its bytes are still in the database file.
        async fn prune_and_scan(label: &str, secure_delete: Option<&str>) -> bool {
            let path = fresh_db(&format!("xc005-{}", label.replace(' ', "-")));
            let (store, pool) = file_store(&path, secure_delete).await;
            let effective = sqlx::query_scalar::<_, i64>("PRAGMA secure_delete").fetch_one(&pool).await.unwrap();
            let journal = sqlx::query_scalar::<_, String>("PRAGMA journal_mode").fetch_one(&pool).await.unwrap();
            if secure_delete.is_none() {
                // Which SQLite is this process actually running, and how was it built?
                let version = sqlx::query_scalar::<_, String>("SELECT sqlite_version()").fetch_one(&pool).await.unwrap();
                let options = sqlx::query_scalar::<_, String>("PRAGMA compile_options").fetch_all(&pool).await.unwrap();
                println!("sqlite_version() = {version}; compile_options mentioning SECURE/THREADSAFE/OMIT: {:?}", options.iter().filter(|o| o.contains("SECURE") || o.contains("THREADSAFE") || o.starts_with("OMIT")).collect::<Vec<_>>());
                println!("all compile_options: {options:?}");
            }

            store.store_keygen_secrets(GROUP, ME, keygen_secrets()).await.unwrap();
            let json = stored_json(&pool).await;
            let marker = secret_marker(&json).to_owned();

            // A reconciliation that retains nothing schedules the row (store.rs:311-345),
            // and the next housekeeping past that block collects it (store.rs:356-375).
            assert!(store.schedule_group_secrets_deletion(100, &RetainedGroups::default()).await.unwrap());
            let pruned = store.prune_scheduled_secrets(100).await.unwrap();
            assert_eq!(pruned.keygen, 1);
            assert!(get_keygen_secrets(&store, GROUP).await.is_none(), "logically deleted");
            pool.close().await;

            let file = std::fs::read(&path).unwrap();
            let present = contains(&file, marker.as_bytes());
            println!(
                "[{label}] PRAGMA secure_delete = {effective}, journal_mode = {journal}: stored JSON {} bytes; after prune the secret's bytes are {} in the {}-byte database file",
                json.len(),
                if present { "STILL PRESENT" } else { "absent (zeroed)" },
                file.len()
            );
            present
        }

        #[tokio::test]
        async fn pruned_secret_bytes_survive_in_the_file_unless_secure_delete_is_on() {
            // 1. Whatever this host's libsqlite3 defaults to (the project sets no pragma).
            let default_present = prune_and_scan("library default", None).await;
            // 2. A libsqlite3 built without SQLITE_SECURE_DELETE.
            let off_present = prune_and_scan("secure_delete OFF", Some("OFF")).await;
            // 3. A libsqlite3 built with it (or the pragma from remediation 1).
            let on_present = prune_and_scan("secure_delete ON", Some("ON")).await;

            assert!(off_present, "without secure_delete the DELETE leaves the secret bytes in place");
            assert!(!on_present, "with secure_delete the freed cell is overwritten");
            println!(
                "erasure is decided by the library flag alone: default-build outcome on this host = {}",
                if default_present { "bytes retained" } else { "bytes zeroed" }
            );
        }

        /// The rollback journal (the default `journal_mode`, store.rs sets none)
        /// receives the page's pre-image -- the secret included -- when the
        /// prune's DELETE runs, and is unlinked (not wiped) at commit, whatever
        /// `secure_delete` says.
        #[tokio::test]
        async fn the_rollback_journal_holds_the_secret_pre_image_during_the_prune() {
            let path = fresh_db("xc005-journal");
            let (store, pool) = file_store(&path, Some("ON")).await;
            store.store_keygen_secrets(GROUP, ME, keygen_secrets()).await.unwrap();
            let marker = secret_marker(&stored_json(&pool).await).to_owned();
            assert!(store.schedule_group_secrets_deletion(100, &RetainedGroups::default()).await.unwrap());

            // The prune's own statement (store.rs:360), held open before its commit.
            let mut tx = pool.begin().await.unwrap();
            sqlx::query("DELETE FROM keygen_secrets WHERE delete_at_block <= ?")
                .bind(100i64)
                .execute(&mut *tx)
                .await
                .unwrap();
            let journal_path = format!("{path}-journal");
            let journal = std::fs::read(&journal_path).expect("rollback journal exists while the transaction is open");
            let in_journal = contains(&journal, marker.as_bytes());
            println!(
                "during the prune: {journal_path} is {} bytes and {} the secret's bytes (secure_delete ON on the main file)",
                journal.len(),
                if in_journal { "CONTAINS" } else { "does not contain" }
            );
            tx.commit().await.unwrap();
            let unlinked = !std::path::Path::new(&journal_path).exists();
            println!("after commit: journal unlinked = {unlinked} (its blocks are released to the filesystem, not overwritten)");
            pool.close().await;

            assert!(in_journal);
            assert!(unlinked);
        }
    }

    mod qa2_cov_7_6 {
        use crate::bindings::{self, Consensus};
        use alloy::{
            primitives::{Address, B256, Bytes, U256},
            sol_types::{SolEvent, SolEventInterface},
        };
        use safenet_core::index::events::Events;

        #[test]
        fn out_of_range_operation_in_transaction_proposed_decodes_to_invalid_not_an_error() {
            let event = Consensus::TransactionProposed {
                safeTxHash: B256::repeat_byte(0x11),
                safeId: B256::repeat_byte(0x22),
                oracle: Address::repeat_byte(0x33),
                epoch: 7,
                oracleData: Bytes::from_static(b"oracle-data"),
                transaction: bindings::SafeTransaction {
                    chainId: U256::from(100),
                    safe: Address::repeat_byte(0x44),
                    to: Address::repeat_byte(0x55),
                    value: U256::ZERO,
                    data: Bytes::from_static(b"calldata"),
                    operation: bindings::Operation::DELEGATECALL,
                    safeTxGas: U256::ZERO,
                    baseGas: U256::ZERO,
                    gasPrice: U256::ZERO,
                    gasToken: Address::ZERO,
                    refundReceiver: Address::ZERO,
                    nonce: U256::from(1),
                },
            };
            let log = event.encode_log_data();
            let topics = log.topics().to_vec();
            let mut data = log.data.to_vec();

            // Non-indexed layout: word 0 `epoch`, word 1 offset of `oracleData`,
            // word 2 offset of the `SafeTransaction` tuple; `operation` is the
            // tuple's sixth head word.
            let tuple = usize::try_from(U256::from_be_slice(&data[64..96])).unwrap();
            let operation = tuple + 5 * 32;
            assert_eq!(data[operation + 31], 1, "DELEGATECALL encodes as 1");
            data[operation + 31] = 2; // out of range for the two-variant enum

            // `decode_raw_log` (the path `watcher_events!` uses, events.rs:577-591): Ok, with `__Invalid`.
            let decoded = Consensus::ConsensusEvents::decode_raw_log(&topics, &data).expect("decodes without error");
            match decoded {
                Consensus::ConsensusEvents::TransactionProposed(event) => {
                    println!("operation byte 2 decoded to {:?}", event.transaction.operation);
                    assert_eq!(event.transaction.operation, bindings::Operation::__Invalid);
                }
                other => panic!("unexpected event {other:?}"),
            }
            // The validator's own `Event::decode_log` accepts the log as well.
            assert!(matches!(
                crate::service::Event::decode_log(&topics, &data),
                Some(crate::service::Event::Consensus(Consensus::ConsensusEvents::TransactionProposed(_)))
            ));
            // Whereas truncated data is a genuine decode error (F2-VAL-061 scenario 3 stands).
            assert!(Consensus::ConsensusEvents::decode_raw_log(&topics, &data[..data.len() - 32]).is_err());
        }
    }
