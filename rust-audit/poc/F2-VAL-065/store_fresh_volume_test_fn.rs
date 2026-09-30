
    #[tokio::test]
    async fn f2_val_065_sample_shaped_database_url_fails_on_a_fresh_volume() {
        // QA2-VAL-B PoC for F2-VAL-065 (temporary; reverted after the run).
        // The sample ships `database = "sqlite:/var/lib/safenet/validator/data/storage.db"`
        // with no `?mode=rwc`. It is parsed with `SqliteConnectOptions::from_str`
        // (config.rs:27-29) and opened by `safenet_core::utils::connect_sqlite`
        // (main.rs:46). sqlx-sqlite 0.9.0 defaults `create_if_missing` to false,
        // so the first start on an existing-but-empty volume directory fails.
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr as _;
        let dir = std::env::var("F2_VAL_065_DIR").expect("F2_VAL_065_DIR: an existing empty dir");
        let path = format!("{dir}/storage.db");
        assert!(!std::path::Path::new(&path).exists(), "fresh volume: no database file yet");

        let sample_shaped = format!("sqlite:{path}");
        let options = SqliteConnectOptions::from_str(&sample_shaped).unwrap();
        let err = safenet_core::utils::connect_sqlite(options)
            .await
            .expect_err("reproduction failed: the sample-shaped URL opened a fresh volume");
        eprintln!("F2-VAL-065 {sample_shaped} -> {err}");
        assert!(format!("{err}").contains("unable to open database file"));
        assert!(!std::path::Path::new(&path).exists(), "nothing was created");

        // The repo's own scripts append `?mode=rwc`; the same path then works.
        let with_rwc = format!("{sample_shaped}?mode=rwc");
        let options = SqliteConnectOptions::from_str(&with_rwc).unwrap();
        let pool = safenet_core::utils::connect_sqlite(options).await.expect("mode=rwc creates the file");
        eprintln!("F2-VAL-065 {with_rwc} -> ok");
        assert!(std::path::Path::new(&path).exists());
        pool.close().await;
    }
