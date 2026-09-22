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
