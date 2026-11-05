    // ======================================================================
    // rv-xc run 3 adaptation of poc/F2-XC-005/qa2_xc_005_store_test.rs
    // (QA2-XC, run 2), module qa2_xc_005 only. Temporarily pasted at the end
    // of `crates/validator/src/secrets/store.rs`'s `mod tests` block; reverted
    // with `git checkout -- crates/validator/src/secrets/store.rs` after the run.
    //
    // API updates at 361aaf3: `schedule_group_secrets_deletion` takes a
    // `BTreeMap<B256, RetainedSecrets>` (an empty map retains nothing); scratch
    // databases get a per-process suffix instead of being deleted.
    // New variant: Direct Nonce (#1033) stores one nonce pair per ceremony in
    // `signing_nonces` and `take_signing_nonces` clears it with an `UPDATE …
    // SET nonces = NULL`; the third test checks whether a taken nonce's bytes
    // stay in the file.
    // ======================================================================
    mod qa2_xc_005 {
        use super::*;
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;

        fn fresh_db(name: &str) -> String {
            let dir = std::env::var("QA2_XC_DIR").expect("QA2_XC_DIR must name a scratch directory");
            let path = format!("{dir}/{name}-{}.db", std::process::id());
            assert!(!std::path::Path::new(&path).exists(), "{path} already exists");
            path
        }

        /// The store over a file-backed pool opened exactly as the validator
        /// opens it (`safenet_core::utils::connect_sqlite`, no pragmas), with an
        /// optional `secure_delete` override standing in for a SQLite built
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
        /// of the secret, short enough to sit inside one database page.
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
                println!(
                    "sqlite_version() = {version}; compile_options mentioning SECURE: {:?}",
                    options.iter().filter(|o| o.contains("SECURE")).collect::<Vec<_>>()
                );
            }

            store.store_keygen_secrets(GROUP, ME, keygen_secrets()).await.unwrap();
            let json = stored_json(&pool).await;
            let marker = secret_marker(&json).to_owned();

            // A reconciliation that retains nothing schedules the row
            // (store.rs:312-355), and the next housekeeping past that block
            // collects it (store.rs:361-386).
            assert!(store.schedule_group_secrets_deletion(100, &BTreeMap::new()).await.unwrap());
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
            // 1. Whatever the linked SQLite defaults to (the project sets no pragma).
            let default_present = prune_and_scan("library default", None).await;
            // 2. A SQLite built without SQLITE_SECURE_DELETE.
            let off_present = prune_and_scan("secure_delete OFF", Some("OFF")).await;
            // 3. A SQLite built with it (or the pragma from remediation 1).
            let on_present = prune_and_scan("secure_delete ON", Some("ON")).await;

            assert!(off_present, "without secure_delete the DELETE leaves the secret bytes in place");
            assert!(!on_present, "with secure_delete the freed cell is overwritten");
            println!(
                "erasure is decided by the library flag alone: default-build outcome = {}",
                if default_present { "bytes retained" } else { "bytes zeroed" }
            );
        }

        /// The rollback journal (the default `journal_mode`, nothing sets
        /// another) receives the page's pre-image, the secret included, when
        /// the prune's DELETE runs, and is unlinked (not wiped) at commit,
        /// whatever `secure_delete` says.
        #[tokio::test]
        async fn the_rollback_journal_holds_the_secret_pre_image_during_the_prune() {
            let path = fresh_db("xc005-journal");
            let (store, pool) = file_store(&path, Some("ON")).await;
            store.store_keygen_secrets(GROUP, ME, keygen_secrets()).await.unwrap();
            let marker = secret_marker(&stored_json(&pool).await).to_owned();
            assert!(store.schedule_group_secrets_deletion(100, &BTreeMap::new()).await.unwrap());

            // The prune's own statement (store.rs:365), held open before its commit.
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
                "during the prune: <scratch>/{} is {} bytes and {} the secret's bytes (secure_delete ON on the main file)",
                std::path::Path::new(&journal_path).file_name().unwrap().to_string_lossy(),
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

        /// New in run 3: a per-ceremony nonce pair taken for signing is cleared
        /// with `UPDATE … SET nonces = NULL` (store.rs:281-285). Without
        /// `secure_delete` the cleared bytes stay in the file, next to a
        /// signature share that is public onchain once published.
        async fn take_and_scan(label: &str, secure_delete: Option<&str>) -> bool {
            let path = fresh_db(&format!("xc005-nonces-{}", label.replace(' ', "-")));
            let (store, pool) = file_store(&path, secure_delete).await;
            let id = signature_id(7);
            assert!(store.store_signing_nonces(GROUP, id, ME, signing_nonces()).await.unwrap().is_some());
            let json = sqlx::query_scalar::<_, String>("SELECT nonces FROM signing_nonces WHERE signature_id = ?")
                .bind(key(id))
                .fetch_one(&pool)
                .await
                .unwrap();
            let marker = secret_marker(&json).to_owned();
            assert!(store.take_signing_nonces(id).await.unwrap().is_some(), "taken once");
            assert!(store.take_signing_nonces(id).await.unwrap().is_none(), "cleared");
            pool.close().await;

            let file = std::fs::read(&path).unwrap();
            let present = contains(&file, marker.as_bytes());
            println!(
                "[nonces, {label}] stored nonce JSON {} bytes; after take the nonce's bytes are {} in the {}-byte database file",
                json.len(),
                if present { "STILL PRESENT" } else { "absent (zeroed)" },
                file.len()
            );
            present
        }

        #[tokio::test]
        async fn taken_signing_nonce_bytes_survive_in_the_file_unless_secure_delete_is_on() {
            let default_present = take_and_scan("library default", None).await;
            let on_present = take_and_scan("secure_delete ON", Some("ON")).await;
            assert!(default_present, "the library default leaves the taken nonce in the file");
            assert!(!on_present, "with secure_delete the cleared value is overwritten");
        }
    }
