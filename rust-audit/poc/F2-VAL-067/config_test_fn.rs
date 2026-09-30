
    #[test]
    fn f2_val_067_short_epoch_relative_to_keygen_timeout_livelocks_rollover() {
        // QA2-VAL-B PoC for F2-VAL-067 (temporary; reverted after the run).
        use crate::consensus::epoch;

        // 1. No cross-validation guard: `blocks_per_epoch` shorter than the
        //    (defaulted) `key_gen_timeout` loads without any error.
        let config = toml::from_str::<Config>(
            r#"
                rpc = "http://localhost:8545"
                signer = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                database = "sqlite::memory:"

                [validator]
                consensus = "0x1111111111111111111111111111111111111111"
                blocks_per_epoch = 100
            "#,
        )
        .unwrap();
        let bpe = config.validator.blocks_per_epoch;
        let kgt = config.validator.key_gen_timeout.get();
        assert_eq!(bpe.get(), 100);
        assert_eq!(kgt, 120, "key_gen_timeout defaulted");
        assert!(
            bpe.get() < kgt,
            "pathological pairing (window shorter than the keygen timeout) is accepted with no complaint"
        );

        // 2. The livelock: a DKG opened at block B (its round deadline is
        //    B + key_gen_timeout) is ABANDONED the moment the block enters the
        //    next epoch window -- the first block whose `next_number` exceeds
        //    B's. That boundary is at most B + blocks_per_epoch, which is
        //    strictly less than the deadline B + key_gen_timeout. So a stalled
        //    participant is never reached by the exclusion deadline; the next
        //    attempt "includes everyone again" and no epoch is ever staged.
        for b in [0u64, 1, 50, 99, 100, 150, 1440, 1739, 100_000] {
            let start_epoch = epoch::next_number(b, bpe);
            let boundary = (b / bpe.get() + 1) * bpe.get();
            assert!(
                epoch::next_number(boundary, bpe) > start_epoch,
                "the window rolls over at the boundary block"
            );
            assert!(boundary <= b + bpe.get());
            assert!(
                boundary < b + kgt,
                "abandonment boundary {boundary} precedes the keygen deadline {}",
                b + kgt
            );
        }
    }
