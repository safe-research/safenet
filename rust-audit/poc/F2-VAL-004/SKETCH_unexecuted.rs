// UNEXECUTED PoC sketch for F2-VAL-004 (deadline divergence, case (a)).
// Paste into a `#[cfg(test)] mod poc_tests` in crates/validator/src/state/keygen.rs
// (same scaffolding as the F2-VAL-003/006 module: transition(), epoch_group(), nz()).
// Command: cargo test -p validator --bins poc_f2_val_004_deadline_divergence
//
// Expected outcome: the late-setup exit derives the share-round deadline as
// (commitment_deadline + key_gen_timeout), while the normal exit derives it as
// (last_commitment_block + key_gen_timeout); asserting the two differ passes.
//
// #[test]
// fn poc_f2_val_004_deadline_divergence() {
//     let t = transition();                 // bpe=100, kgt=20
//     let group2 = epoch_group(2);
//     let (count, threshold) = group2.size();
//     let commitment_deadline = 30u64;      // deadline the round was opened with
//
//     // Build CollectingCommitments{gid2} with ALL `count` peer commitments
//     // already collected but this validator's own secrets still None, then
//     // deliver Resume::Setup -> handle_key_gen_setup takes the late exit.
//     // Assert the resulting CollectingShares.deadline == commitment_deadline + kgt.
//     let late = drive_late_setup(&t, &group2, commitment_deadline);   // = 50
//
//     // The normal exit: the same commitments arrive one-by-one and the LAST
//     // KeyGenCommitted at block `b_last` closes the round.
//     // Assert CollectingShares.deadline == b_last + kgt.
//     let b_last = 12u64;
//     let normal = drive_normal_close(&t, &group2, commitment_deadline, b_last); // = 32
//
//     assert_ne!(late, normal, "honest nodes derive different share deadlines");
// }
//
// Basis: crates/validator/src/state/keygen.rs:130-140 (late) vs 247-252 (normal),
// and the doc note at 1232-1236 that the two exits "do not agree on a block".
// Not executed: the trigger is a reorg-replay-vs-effect ordering race (Critic
// Plausible, 50); a pure test proves only the deadline divergence (a), not that
// the race actually fires, so it would not lift the finding above Plausible.
