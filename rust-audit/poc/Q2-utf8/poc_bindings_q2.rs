
// ==== V-CORE-SEN Q2 PoC BEGIN ====
#[cfg(test)]
mod poc_q2_utf8 {
    use alloy::{
        hex,
        primitives::{B256, b256},
        sol_types::{SolEvent as _, SolEventInterface as _},
    };
    use crate::bindings::oracle::SentinelOracle::{
        DisputeOutOfScope, DisputeResolved, Revealed, SentinelOracleEvents,
    };
    use safenet_core::index::events::Events as _;
    use crate::bindings::SentinelEvents;

    const REQ: B256 =
        b256!("0x00000000000000000000000000000000000000000000000000000000000000aa");
    const SENT: B256 =
        b256!("0x000000000000000000000000f39fd6e51aad88f6f4ce6ab8827279cfffb92266");

    #[test]
    fn q2_non_utf8_reason_decode() {
        // Revealed(bytes32 indexed requestId, address indexed sentinel,
        //          bool approved, uint96 bondAmount, string reason)
        let topics = [Revealed::SIGNATURE_HASH, REQ, SENT];
        // approved = true; bondAmount = 0; offset = 0x60; len = 1; byte = 0x80
        let data = hex::decode(concat!(
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000060",
            "0000000000000000000000000000000000000000000000000000000000000001",
            "8000000000000000000000000000000000000000000000000000000000000000",
        ))
        .unwrap();

        let decoded = SentinelOracleEvents::decode_raw_log(&topics, &data);
        println!("Revealed / SolEventInterface::decode_raw_log => {decoded:?}");

        // Also through the exact path safenet-core uses (`watcher_events!` ->
        // `Events::decode_log`, crates/core/src/index/events.rs:546-554).
        let via_core = SentinelEvents::decode_log(&topics, &data);
        println!("Revealed / watcher_events!::decode_log       => {via_core:?}");

        assert!(
            decoded.is_ok(),
            "non-UTF-8 `reason` is REJECTED by alloy-sol-types 1.6.0 — F-SEN-013 is High"
        );
        assert!(
            via_core.is_some(),
            "non-UTF-8 `reason` yields None from watcher_events!::decode_log, which \
             aborts the whole batch (events.rs:495-516) — F-SEN-013 is High"
        );
    }

    #[test]
    fn q2_non_utf8_dispute_contexts_decode() {
        // DisputeResolved(bytes32 indexed requestId, RequestState outcome,
        //                 uint128 slashed, string context)
        let topics = [DisputeResolved::SIGNATURE_HASH, REQ];
        let data = hex::decode(concat!(
            "0000000000000000000000000000000000000000000000000000000000000003", // RESOLVED_APPROVED
            "0000000000000000000000000000000000000000000000000000000000000000", // slashed
            "0000000000000000000000000000000000000000000000000000000000000060", // offset
            "0000000000000000000000000000000000000000000000000000000000000001", // len
            "8000000000000000000000000000000000000000000000000000000000000000", // 0x80
        ))
        .unwrap();
        let a = SentinelOracleEvents::decode_raw_log(&topics, &data);
        println!("DisputeResolved.context  => {a:?}");

        // DisputeOutOfScope(bytes32 indexed requestId, string context)
        let topics2 = [DisputeOutOfScope::SIGNATURE_HASH, REQ];
        let data2 = hex::decode(concat!(
            "0000000000000000000000000000000000000000000000000000000000000020",
            "0000000000000000000000000000000000000000000000000000000000000001",
            "8000000000000000000000000000000000000000000000000000000000000000",
        ))
        .unwrap();
        let b = SentinelOracleEvents::decode_raw_log(&topics2, &data2);
        println!("DisputeOutOfScope.context => {b:?}");

        assert!(a.is_ok() && b.is_ok(), "an arbitrator-supplied non-UTF-8 context is rejected");
    }
}
// ==== V-CORE-SEN Q2 PoC END ====
