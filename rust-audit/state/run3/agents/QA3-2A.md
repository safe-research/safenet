# QA3-2A coverage log (second reproducer, run 3)

Worktree wt/rv-core-b, env rv-core-b.sh, Anvil ports 31300-31309 (none used: all PoCs are validator-crate unit tests). Commit 361aaf3. No memory writes (parallel instances). Independence: read each finding only to and including its Critic section before writing my PoC; used R6's cited logs (state/run3/logs) for the harness shape, not any primary QA PoC.

All PoCs are temporary `#[cfg(test)]` modules appended to a tracked file in the worktree, run, then reverted with `git checkout --`; ended with clean `git status --short`. Build cache warm; each incremental build about 5-8 s.

## F3-VAL-030 (High): Reproduced (root cause, basis 1-4)

- Route: real signing state machine (sign.rs), different from the primary's two-validator Anvil reorg. Real 3-of-5 FROST DKG for key material; validator placed in WaitingForOracle; approving OracleResult -> GenerateNonces same block (deadline 1006, no max_reorg_depth), threshold nonce commits -> UseNonces, resume -> SignShare with attestTransaction callback, expires_at 1007.
- Command: `cargo test --offline -p validator --bins qa2a_val030 -- --nocapture`. PoC poc/F3-VAL-030/repro-2/.
- Bite: remediation 1 (hold approval) -> no GenerateNonces in-window -> assertion fails. Sound.
- Not executed (cited): reorg rollback (basis 5), queue rebroadcast (basis 6), contract accept (basis 7-9, guard = settled design). Certainty 85.

## F3-XC-100 (High): Reproduced

- Route: two victims (last committer cancels c[1] and c[2]), different attacker position from one-victim. Real keygen accepts crafted commitment; both victims' y = (0,0), survivors real; state machine excludes {attacker, V1, V2} -> EpochSkipped(2).
- Command: `cargo test --offline -p validator --bins qa2a_xc100 -- --nocapture`. PoC poc/F3-XC-100/repro-2/.
- Bite: remediation 1 (detect identity verifying share, restart excluding last committer) -> poisoned group never reaches CollectingShares -> assertion fails. Sound. Certainty 92.

## F3-VAL-062 (Medium): Reproduced

- Route: all five shares dropped by the warp (vs single dropped), restarted validator skips the epoch itself; live-order control shows the drop and the 1680-vs-1561 deadline drift are ordering-dependent.
- Command: `cargo test --offline -p validator --bins qa2a_val062 -- --nocapture`. PoC poc/F3-VAL-062/repro-2/.
- Bite: remediation 1/3 (buffer shares during the setup wait, replay on finalize) -> CollectingConfirmations -> assertion fails. Sound for the drop; drift needs the deadline-source part of option 1. Certainty 90.

## Not attempted

- No live RPC, no Anvil runs needed. The full end-to-end Anvil reorg for F3-VAL-030 is the primary QA's route; I took the state-machine root-cause route instead and was explicit about the cited downstream halves.
