// UNEXECUTED PoC sketch for F2-VAL-007 (stale active_epoch -> phantom rollover session).
// Paste into a `#[cfg(test)] mod poc_tests` in crates/validator/src/state/keygen.rs.
// Command: cargo test -p validator --bins poc_f2_val_007_stale_active_epoch
//
// Expected outcome: after the validator misses epoch k+1 (active_epoch stays k),
// confirming k+2 builds the rollover hash from active_epoch=k (not the chain's
// k+1) and opens a WaitingForRequest signing session keyed by that wrong hash
// with epoch k's key share/signers.
//
// #[test]
// fn poc_f2_val_007_stale_active_epoch() {
//     let t = transition();
//     // 1. Foreign EpochStaged{k+1} while NOT in SigningRollover -> ignored;
//     //    active_epoch remains k (keygen.rs:636-645).
//     // 2. State with active_epoch=k, epochs={k: Epoch{group_k, key_share_k,..}},
//     //    rollover=CollectingConfirmations for epoch k+2 with all-but-one confirmed.
//     // 3. Feed the final KeyGenConfirmed for k+2.
//     let hash_stale = t.consensus.epoch_rollover_hash(EpochId::Number{number:k}, k2, block, &gk);
//     let hash_real  = t.consensus.epoch_rollover_hash(EpochId::Number{number:k1}, k2, block, &gk);
//     // assert the session inserted under `after.signing` is keyed by hash_stale,
//     // not hash_real, and carries epoch k's key_share/group_id (keygen.rs:516-547).
//     assert!(after.signing.contains_key(&hash_stale));
//     assert!(!after.signing.contains_key(&hash_real));
// }
//
// Basis: crates/validator/src/state/keygen.rs:930-946 (active_epoch advances only
// from EpochStaged), 636-645 (foreign stage ignored), 516-547 (session from
// active_epoch). Not executed: Low severity, downstream of F2-VAL-003/004/005;
// the stale-hash mechanism is already E2-Confirmed and a pure test would confirm
// only the hash/session, not the onchain junk-Sign step (which depends on R5's
// signing timeout path).
