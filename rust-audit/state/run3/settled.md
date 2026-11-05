# Run 3: settled and known items

Read this before filing anything. It replaces the earlier runs' material for blind run-3 agents: it lists what the team has already decided, what is already known, and the real parameters to use. It names behaviours, not earlier finding IDs.

## 1. Settled: do not file

| Item | Source |
| --- | --- |
| Missing or default HTTP timeout on the sentinel's engine client. Pre-production hardening at most, Informational. | Team decision (rmeissner, PR #610) |
| Arbitrated transactions are never attested. By design. | Safenet Charter Article I |
| Contract changes after the audited commit `0be96367` are unaudited. A process note, never a code defect. | Team decision |
| Migration, backward compatibility, old databases, old snapshots, old configs. Every Aegis node starts fresh. | Aegis release rule |
| Behaviour that bites only on chains other than Gnosis (Sepolia, Ethereum mainnet, 60M-gas chains). The transaction queue only targets the contract-hosting chain. | Maintainers' review notes, section 1 |
| Testnet timing values that differ from production. Deliberate. | Maintainers' review notes, section 1 |
| 7702 executor review lows L1 to L5: (L1) the startup code check accepts a `0xef0100` delegation designator as executor code; (L2) a missing `eth_getProof` result stalls silently; (L3) a pending authorization is not counted as delegated, so the queue drains for about 16 blocks after a Batched-to-Direct restart; (L4) a reverted batch loses all its actions, with no receipt check; (L5) the batching gas constants have no worst-case test. | Team decision |
| Genesis-ceremony liveness alone (A16): Informational, tagged `known`. Anything reaching a rollover or a later epoch keeps its severity. | PROMPT.md A16 |
| Triggers that need out-of-band database access: restore, copy, manual edit (A17). | PROMPT.md A17 |
| A `Sign` event for another group dropping a pending signing session. Accepted risk. | rmeissner on PR #905, confirmed by remedcu 2026-10-08 |
| Operator-facing warning text that is deliberately alarming (for example the warning about nodes that omit log timestamps). Not a wording defect. | nlordell on PR #1051 |

## 2. Known: file only if you can add something, and tag `known` (A12)

| Item | Where |
| --- | --- |
| The process exits with status 0 after a fatal driver error, so an on-failure restart policy never restarts it. | `crates/validator/src/main.rs`, `crates/sentinel/src/main.rs` |
| Unverified: a validator restarted more than `max_reorg_depth` blocks after a reorg that rolled its group back into DKG can prune its own key-gen secrets during replay and log "permanently halted". | maintainers' review notes, section 8 (PR #1037 follow-up) |
| The ECDH share pad is the raw x-coordinate of the shared point, the same in both directions. The team chose the proof of possession (#1027) and closed the KDF change (#1026). | `crates/validator/src/frost/` |
| Repeated nonce commitments are logged at `warn`. | `crates/validator/src/state/sign.rs` |
| A key-gen secret leaves the secret store by two routes (`take` documentation). | `crates/validator/src/secrets/store.rs` |
| A SQLite reconcile write runs for every warped block (performance). | PR #1017 follow-up |
| Cancel safety of the futures raced in `Driver::next_input`, and of the event-timestamp header fetch on warp pages (#1050 has no cancellation test). | `crates/core/src/driver.rs`, `crates/core/src/index/` |
| A bond can be stranded at `revealDeadline + 1` in the sentinel's deadline handling. | PR #1017 follow-up |
| Doc typo "selecting the for automatic". | `crates/core/src/index/blocks.rs` |
| Oracle fees are paid for results the validators never act on: a result landing after `oracle_timeout` is ignored, yet the sponsor pays and sentinels collect. Recorded as a known issue by #1062. | `crates/validator/src/state/sign.rs:553-565` |
| In-flight engine checks are lost on a restart, or on a live reorg that drops a resume (#973); a commit seen while the request still waits for its engine check is ignored and no reveal follows (#961, testnet incident). | `crates/sentinel/src/`, issues #961 and #973 |
| Timeouts return all bonds and fees; kept as is for the initial version (#549, rmeissner 2026-08-03). | issue #549 |
| TODO and FIXME markers in scope, and the other open GitHub issues on the Rust services (#20, #69, #113, #465). | `state/run3/map.md` section 5 |

## 3. Parameters for triggers and PoCs

Use these, never invented numbers. Phase 0 re-checks the defaults against the audit commit in `state/run3/baseline.md`; where that file differs, it wins.

- Chain: Gnosis (chain 100), 5 s blocks, about 17M block gas, EIP-7825 per-transaction cap 2^24 = 16,777,216 gas. Block visibility p90 about 1.6 s after slot time (issue #926).
- Aegis production: 5 genesis validators, threshold 3 of 5. SentinelOracle fee token USDC.e (6 decimals), fee 0.40, bond 2000x the fee, slashing 1x, DAO share 0 (`contracts/.env.aegis.sample`).
- Validator defaults (checked at `361aaf3`, `state/run3/baseline.md` section 6): `blocks_per_epoch` 1440, `key_gen_timeout` 120, `signing_timeout` 6, `oracle_timeout` 24 (`crates/validator/src/config.rs:89-91`, changed by #1062).
- Sentinel: the sample sets `voting_window` 6 (`crates/sentinel/sentinel.sample.toml:41`, #1062), which gives an engine timeout of 18.75 s on Gnosis (`crates/sentinel/src/main.rs:53-65`).
- Aegis production SentinelOracle windows: commit 6 blocks, reveal 6 blocks, arbitration timeout 483,840 blocks (`contracts/.env.aegis.sample:18-24`). Testnet values (commit 2, reveal 3, arbitration 50,400) differ on purpose.
- The preprocessing nonce chunk (1024) no longer exists: signing nonces are per ceremony since the Direct Nonce work (#1032 to #1044).
- Nothing is in production yet; findings are judged against the Aegis production parameters above.
