    // ---------------------------------------------------------------------
    // Run 3 re-validation PoC (rv-core-b, audit commit 361aaf3) for F-CORE-066,
    // adapted from `poc/F2-CORE-065/validator_config_tests.rs`: the validator's
    // real TOML loader accepts zero and NaN `[transactions]` values. Paste
    // before the closing brace of `mod tests` in `crates/validator/src/config.rs`;
    // revert afterwards. Asserts the defect, so a PASS reproduces it.
    // ---------------------------------------------------------------------
    #[test]
    fn rv3_f_core_066_toml_accepts_zero_and_nan_transaction_values() {
        let config = toml::from_str::<Config>(
            r#"
                rpc = "http://localhost:8545"
                signer = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                database = "sqlite::memory:"

                [validator]
                consensus = "0x1111111111111111111111111111111111111111"

                [transactions]
                max_in_flight_transactions = 0
                blocks_before_resubmit = 0
                priority_fee_cap_percentage = nan
            "#,
        )
        .unwrap();
        let transactions = &config.driver.transactions;
        println!("parsed [transactions]: {transactions:?}");
        assert_eq!(
            transactions.mode,
            safenet_core::tx::SubmissionMode::Direct {
                max_in_flight_transactions: 0
            }
        );
        assert_eq!(transactions.blocks_before_resubmit, 0);
        assert!(transactions.priority_fee_cap_percentage.unwrap().is_nan());
    }
