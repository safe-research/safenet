# Phase 4, CA3: final confidence and severity

Audit commit `361aaf3`. Worktrees `wt/rv-sen` (Anvil 31500 to 31509) and `wt/rv-val` (31400 to 31409), both rebuilt at `361aaf3` before any binary run (each target directory still held a patched bite build from Phase 3). Every in-scope finding file has a `## Confidence (CA3, 2026-10-09)` section, and its Status, Severity and Certainty cells are final. Evidence: `rust-audit/poc/<id>/ca3/`. Medium and higher rows were re-run by the primary QA route and the second reproduction; Low and Informational rows were checked against their evidence files. Reproductions are counted as runs that ended in the claimed final state (balances, commitments, attestations, exit status), not log text.

| ID | practicality or verification result | recommended or final severity | certainty | reason |
| --- | --- | --- | --- | --- |
| F3-SEN-001 | Reproduced: real binary Direct mode 2 of 2 (approve out of gas, commit reverts, no `Committed`); isolated forge 1 of 1 (55,819 needed); Batched control commits, reveals, claims | High | 97 | agree with PC3; documented Aegis config is Direct mode |
| F-SEN-008 | Reproduced (gas half): probe 1 of 1 plus `F3-SEN-001`; USDT-family half not applicable to USDC.e | High (ledger Low) | 95 | same defect as `F3-SEN-001`; report one row |
| F3-SEN-002 | Reproduced: FSM 1 of 1, adversarial real binaries 1 of 1; honest variant: undercount 4 of 4, bond loss 0 of 4 | Medium | 90 | agree on band; variant (a) is conditional on reveal order, not practical as PC3 says |
| F-SEN-002 | Reproduced through the transition 1 of 1; mirror case recovers (#964) | Medium (ledger High) | 90 | one band with `F3-SEN-002` |
| F3-SEN-003 | Reproduced: FSM 1 of 1, real binaries 1 of 1 (800 locked, untracked result) | Medium, `known` | 95 | agree with PC3 |
| F3-SEN-004 | Reproduced: real `StateMachine` 1 of 1, real restart 1 of 1 (own commit ignored, no reveal) | Medium, `known` | 95 | agree with PC3 |
| F-SEN-015 | Not fixed in the restart window: real binary 16 of 16 inside it (10 engine down, 6 flipped), control 0 of 4; emulation 278 of 300; live-reorg variants 5 of 5 fixed | Medium (ledger High) | 95 | blocks deletion; agree with PC3 and CA3-FIX |
| F3-SEN-060 | Sentinel half reproduced 1 of 1; contract half not re-run (fork setup needs RPC), stands on LT3 and code | Low | 85 | agree with PC3 |
| F3-VAL-030 | Reproduced 4 of 4: `SimpleOracle` 1 of 1; `SentinelOracle` at Aegis windows and `max_reorg_depth` 5, depth-4 reorg, 2 of 2 (`DisputeTriggered`, `FROZEN`, `TransactionAttested`); state machine 1 of 1 | High | 97 | PC3's settling check done and positive |
| F3-XC-100 | Reproduced 3 of 3 (QA3-VAL transition, QA3-2A two victims, L-crypto forge `NotOnCurve`) | High | 95 | agree with PC3 |
| F3-VAL-062 | Reproduced 2 of 2 (warp order drops shares; all-shares route `EpochSkipped`) | Medium | 90 | agree; process restart traced, not run |
| F3-VAL-100 | Reproduced 1 of 1 (LT3 state machine) | Low (reviewer Medium) | 88 | agree with PC3 and LT3 |
| F-VAL-060 | Mechanism reproduced 1 of 1; Aegis oracle bytecode and topics re-checked (no foreign topic0, no delegatecall or create, no anonymous events) | Medium (ledger High conditional) | 90 | agree with PC3; theoretical on Aegis, latent key-share recovery |
| F3-CORE-001 | Reproduced 2 of 2 (driver: 5 limiter shapes stall, controls follow; real binary stalls with `/health` OK) | Medium | 85 | agree with PC3 |
| F3-CORE-002 | Reproduced 3 of 3 (unit; driver windows of 500 ms and more lost; real binary loses the proposal) | Medium (reviewer Low) | 80 | agree with PC3; Nethermind error-code change spot-checked at source |
| F3-CORE-030 | Reproduced 2 of 2 (`BadUpdate`, exit 0) | Low (reviewer Medium) | 95 | agree with PC3; documented `docker run` has no restart policy |
| F3-XC-160 | Reproduced: watcher 2 routes, end-to-end injection 1 of 1 (no reveal, 799.60 left), control claims | Medium, `known` consequence | 80 | agree with PC3 |
| F3-CORE-032 | Evidence holds; live one-block route executed through `F3-XC-160`'s run | Medium, `known` (reviewer Low) | 93 | disagree with PC3's Low: the live one-block reorg route (daily on Gnosis) is unweighed; same consequence as `F3-XC-160` and `F3-SEN-004` |
| F3-CORE-004 | Evidence holds | Low | 92 | agree |
| F3-CORE-005 | Evidence holds | Low | 92 | agree |
| F3-CORE-060 | Evidence holds | Low | 75 | agree |
| F3-CORE-061 | Evidence holds | Informational | 90 | agree |
| F3-VAL-031 | Evidence holds (impact rule E1; reorg trigger traced) | Low | 60 | agree |
| F3-VAL-032 | Evidence holds | Informational | 90 | agree |
| F3-VAL-061 | Evidence holds | Low | 90 | agree |
| F3-XC-003 | Evidence holds | Informational | 95 | agree |
| F3-XC-004 | Evidence holds | Informational, `known` | 90 | agree |
| F3-XC-005 | Evidence holds | Low | 95 | agree |
| F3-XC-006 | Evidence holds; seen again in CA3's `F3-SEN-002` runs | Informational | 95 | agree |
| F3-XC-007 | Evidence holds | Informational | 90 | agree |
| F3-XC-030 | Evidence holds | Low | 85 | agree |
| F3-XC-040 | Evidence holds | Informational (reviewer Low) | 90 | agree |
| F3-XC-130 | Evidence holds | Informational, `known` (reviewer Low) | 95 | agree |
| F3-CORE-003 | Duplicate of `F3-CORE-030` | follows `F3-CORE-030` | n/a | pointer |
| F3-CORE-031 | Duplicate of `F3-XC-130` | follows `F3-XC-130` | n/a | pointer |
| F3-VAL-060 | Duplicate of `F3-XC-005` | follows `F3-XC-005` | n/a | pointer |
| F3-SEN-005 | Duplicate of `F3-XC-005` | follows `F3-XC-005` | n/a | pointer |
| F3-XC-001 | Duplicate of `F3-SEN-002` | follows `F3-SEN-002` | n/a | pointer |
| F3-XC-002 | Duplicate of `F3-SEN-003` | follows `F3-SEN-003` | n/a | pointer |
| F3-VAL-001 | Duplicate of `F3-XC-100` | follows `F3-XC-100` | n/a | pointer |

## Cross-run bands (for the reconciliation agent)

| Pair | One band | Why |
| --- | --- | --- |
| `F-CORE-001` (High), `F2-CORE-001` (Medium), `F3-CORE-004` (Low) | Low | the trigger is a reorg deeper than `max_reorg_depth` or a node on a minority fork for more than 5 blocks, outside Section 8's High conditions and not seen on Gnosis; impact is one diverged node |
| `F-CORE-002` (High), `F2-CORE-002` (Medium), `F3-CORE-002` (Medium) | Medium | a provider behaviour A4 names, not attacker input; live on Nethermind 1.32 to 1.35 and 1.39+ |
| `F-CORE-031` (High), `F3-CORE-032` (PC3 Low) and `F-SEN-001` (High), `F3-SEN-004` (Medium) | Medium, `known`, for all four | a lost reveal or lost effect after a rollback (restart or live one-block reorg), recoverable by hand; not High (no attacker, no permanent loss), not Low (daily one-block orphans on Gnosis, every restart in a commit phase) |

Spot-checks of verified-correct claims: PC3's deployed oracle set (`docs/configuration.md:87`), the restart-policy-free `docker run` lines (`docs/validator-handbook.md:124-128`, `docs/sentinel-handbook.md:85-90`), `expires_at: None` on the dispute-timeout actions (`crates/sentinel/src/service.rs:477-507`) and the Changed-row census all hold. CA3-FIX's `F2-SEN-010` and `F-SEN-003` Fixed confirmations: its three variants pass at `361aaf3` (`state/run3/logs/CA3-fixed-spotcheck.txt`).
