
    // ---------------------------------------------------------------------
    // QA2-CORE PoC for F2-CORE-065 (temporary edit; reverted after the run):
    // the TOML loader accepts zero and NaN `[transactions]` values as-is.
    // ---------------------------------------------------------------------
    #[test]
    fn qa_f2_core_065_toml_accepts_zero_and_nan_transaction_values() {
        let config = toml::from_str::<Config>(
            r#"
                    rpc = "https://eth.llamarpc.com"
                    signer = "0x0000000000000000000000000000000000000000000000000000000000000001"
                    database = "sqlite:validator.db"

                    [validator]
                    consensus = "0x0000000000000000000000000000000000000000"

                    [transactions]
                    max_in_flight_transactions = 0
                    blocks_before_resubmit = 0
                    priority_fee_cap_percentage = nan
                "#,
        )
        .unwrap();
        let transactions = &config.driver.transactions;
        println!("parsed [transactions]: {transactions:?}");
        assert_eq!(transactions.max_in_flight_transactions, 0);
        assert_eq!(transactions.blocks_before_resubmit, 0);
        assert!(transactions.priority_fee_cap_percentage.unwrap().is_nan());
    }
