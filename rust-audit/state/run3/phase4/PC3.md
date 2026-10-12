# Phase 4, PC3: practicality against the Aegis production deployment

Audit commit `361aaf3`. Each row has a `## Practicality (PC3, 2026-10-09)` section in its finding file with the actor, cost, condition, the gate it fails, the intended-route check and the cheapest change that would settle any doubt. Parameters: Gnosis 5 s blocks, 5 validators at 3 of 5, oracle windows 6 and 6, arbitration timeout 483,840, USDC.e fee 0.40 and bond 800, `oracle_timeout` 24, `max_reorg_depth` 5, the published sample configurations and `docs/configuration.md`. The certainty column is PC3's confidence in the practicality call, not the finding's certainty; CA3 sets the final certainty and severity.

Seam that applies to every sentinel row marked "after the `F3-SEN-001` fix": on Aegis a Direct-mode sentinel never commits (USDC.e `approve` out of gas), so the bond-loss rows are reachable today only in Batched mode, and in every mode once `F3-SEN-001` is fixed. The calls below assume the fix, since it is one constant.

| ID | practicality or verification result | recommended or final severity | certainty | reason |
| --- | --- | --- | --- | --- |
| F3-SEN-001 | practical: deterministic on the documented Aegis sentinel configuration (Direct mode, USDC.e), every request | High | 95 | no vote, no `OracleResult`, so nothing oracle-gated is attested at launch; no attacker needed; Batched mode is the workaround |
| F-SEN-008 | practical for the 55,000 gas limit; the USDT-family half is theoretical on USDC.e | High (ledger Low, rv-sen High) | 95 | same defect as `F3-SEN-001`; report one row |
| F3-SEN-002 | practical (honest engines one block apart) after the `F3-SEN-001` fix; conditional for a malicious registered sentinel (QA3-2B executed it end to end) | Medium | 85 | funds locked, not lost (manual `claim` or one `finalize`); High defensible for the registered-sentinel variant |
| F-SEN-002 | as `F3-SEN-002` (same defect; #964 recovers the mirror case when the uncounted committer reveals) | Medium (ledger High) | 85 | one band for both rows |
| F3-SEN-003 | practical after the `F3-SEN-001` fix: any 30 s outage across the reveal window | Medium, `known` | 90 | 799.60 USDC.e locked per occurrence until a manual `claim`; `RevealMissed` recovery unreachable |
| F3-SEN-004 | practical after the `F3-SEN-001` fix: an ordinary restart of 20 s or more during a request's commit phase | Medium, `known` | 90 | widest lost-reveal path; recoverable by hand |
| F3-SEN-060 | conditional: frozen dispute, Circle blacklists the sponsor (or pauses USDC.e) at the deadline, 28 days without a ruling, later unblacklisting; fails gate 3 | Low | 85 | external privileged trigger; recoverable by a manual `claim` |
| F-SEN-015 | conditional: a restart whose warp replays the proposal below the new `safe` with the own commitment after it (a window of about one block per block of commit latency) | Medium (ledger High; CA3-FIX input Medium) | 85 | live-reorg route fixed; restart window remains, recoverable |
| F3-VAL-030 | conditional: a natural reorg of 3 or more blocks at a vote edge plus a split vote with a late deny; none of 3 or more is verified on Gnosis in 294.8 days of explorer data (95 % bound about 3.7 a year), timely 2-block reorgs about every 150 days (researcher consult) | High | 85 | breaks the no-attestation-without-approval rule inside A5's handled bound; guard path needs no re-inclusion; cheap fix; revisit at GIP-153 (2 s blocks, around 2027-01) |
| F3-CORE-004 | conditional: a reorg deeper than 5 blocks (none verified; needs a finality failure past about 32 slots) or a node that followed a minority fork for more than 5 blocks | Low | 85 | one diverged node, no key exposure; run-1 twin `F-CORE-001` (ledger High) and `F2-CORE-001` should take this practicality |
| F3-VAL-031 | conditional: a natural 1-block orphan of exactly block d - 1 (Gnosis orphans 0.03 to 0.08 % of slots) seen by exactly two of four honest validators, with a commitment the replacement lacks | Low | 85 | one ceremony lost, recoverable by a new proposal |
| F3-VAL-100 | conditional: a reorg of 2 or more blocks in a window of a few blocks that opens once per deployment (about one timely 2-block reorg per 150 days, so roughly one in a million per deployment) | Low (reviewer Medium, LT3 Low) | 85 | one validator sits out epoch 1; not A16-exempt because it reaches the epoch-1 rollover |
| F3-XC-100 | conditional: one Byzantine or key-compromised validator of the five (inside A2) | High | 90 | every rollover skipped, blame on the honest victim; attestations continue |
| F3-VAL-001 | duplicate of `F3-XC-100` | High | 90 | follows the canonical |
| F3-VAL-032 | theoretical: documentation only, the code is safe | Informational | 95 | fails gate 4 |
| F3-VAL-061 | theoretical as an attack; conditional as operator configuration drift | Low | 90 | fails gate 3 (operator only) |
| F3-VAL-062 | conditional: a restart that stops in the first 20 to 25 s after an epoch boundary with 30 s or more downtime (about 0.35 % of random restarts) | Medium | 85 | one validator sits out an epoch; two skip it; attestations continue |
| F3-CORE-001 | conditional: an RPC plan below about 133 `eth_getLogs`/s, which covers every published free or public Gnosis tier (researcher consult), plus any transient failure; self-hosted nodes and large paid plans do not stall | Medium | 85 | a transient error latches into a permanent stall with `/health` OK; a shared endpoint latches every node on it at once |
| F3-CORE-002 | conditional: a backend that answers an empty success for a just-seen block for more than about 0.4 s: Nethermind 1.32 to 1.35, and again 1.39+ for a processed but not yet canonical hash (source-verified by the consult) | Medium | 80 | the handbook's "before 1.36" premise is stale; twins `F-CORE-002` (ledger High) and `F2-CORE-002` should take this band; the default-path twin `F2-CORE-003` (ledger Informational) needs a fresh look |
| F3-CORE-030 | conditional: a restart while the RPC head is 6 or more blocks behind; reachable through dRPC's balancer, which keeps Gnosis upstreams up to 10 blocks behind in rotation | Low (reviewer and Critic Medium) | 80 | loud, at an operator-started restart, no state loss; the documented `docker run` has no restart policy; run-2 twin `F2-CORE-007` is Low |
| F3-XC-160 | conditional: an endpoint that mixes Reth or Nethermind 1.35 and older backends (or answers rate limits with -32001, like 1RPC) at different heights; Erigon and Nethermind 1.36+ answer -32000 and close the path | Medium, `known` consequence | 85 | per-request bond lock, recoverable by a manual `claim`; remediation 1 is cheap |
| F3-CORE-005 | conditional: host clock several seconds behind, self-inflicted | Low | 90 | fails gate 3 |
| F3-CORE-032 | conditional: a restart whose restore-point block holds a proposal the sentinel committed to | Low, `known` | 85 | rare per restart; `F3-SEN-004` is the wider window; run-1 twins `F-CORE-031` (ledger High) and `F-SEN-001` should take the same practicality |
| F3-CORE-060 | conditional: an accepted transaction left unincluded for minutes; no attacker route | Low | 85 | small signer balance caps the money; liveness once drained |
| F3-CORE-061 | theoretical: operator-only values | Informational | 95 | self-inflicted |
| F3-XC-003 | theoretical: packaging comments, no runtime effect | Informational | 95 | fails gate 4 |
| F3-XC-004 | theoretical for the documented co-deployed engine | Informational, `known` | 90 | #1046 trade-off |
| F3-XC-005 | practical first-start failure for sample users, not material | Low | 90 | loud, before any chain write, one edit |
| F3-XC-006 | practical, not material | Informational | 95 | revert gas and a queue slot |
| F3-XC-007 | theoretical: no advisory reachable (E1 probes) | Informational | 95 | process gap only |
| F3-XC-030 | conditional: a hard failure inside a millisecond window per warp page | Low | 85 | close to theoretical |
| F3-XC-040 | theoretical: operator pastes the key into a typed field | Informational | 90 | fails gate 3 |
| F3-XC-130 | theoretical: operator typo, own logs | Informational, `known` | 95 | fails gate 3 |
| F-VAL-060 | theoretical on Aegis: the only listed oracle is the immutable `SentinelOracle`, which has no `DELEGATECALL` and emits 16 events with no `topic0` shared with `Coordinator` or `Consensus` (E1, `poc/F-VAL-060/pc3/`) | Medium (ledger High conditional) | 90 | precondition settled false for Aegis; keep as a one-line hardening fix |
| F3-CORE-003 | duplicate of `F3-CORE-030` | follows `F3-CORE-030` | n/a | pointer section |
| F3-CORE-031 | duplicate of `F3-XC-130` | Informational, `known` | 95 | pointer section |
| F3-VAL-060 | duplicate of `F3-XC-005` | Low | 90 | pointer section |
| F3-SEN-005 | duplicate of `F3-XC-005` | Low | 90 | pointer section |
| F3-XC-001 | duplicate of `F3-SEN-002` | Medium | 85 | pointer section |
| F3-XC-002 | duplicate of `F3-SEN-003` | Medium, `known` | 90 | pointer section |

## Notes for CA3 and the reconciliation

Same defect, different bands across runs. The practicality above applies to both rows of each pair, so the final report should carry one band per defect:

| Run-3 row (PC3 band) | Earlier row (ledger band) | Practicality that decides it |
| --- | --- | --- |
| `F3-CORE-004` (Low) | `F-CORE-001` (High), `F2-CORE-001` | no reorg deeper than 5 blocks verified on Gnosis; finality bounds it at about 32 slots |
| `F3-CORE-002` (Medium) | `F-CORE-002` (High), `F2-CORE-002` | provider empty-answer window above about 0.4 s; live again on Nethermind 1.39+ |
| `F3-CORE-032` (Low, `known`) and `F3-SEN-004` (Medium, `known`) | `F-CORE-031` (High), `F-SEN-001` (High) | lost reveal after a restart, recoverable by a manual `claim` |
| `F3-CORE-030` (Low) | `F2-CORE-007` (Low) | loud exit at an operator-started restart, no state loss |
| `F3-SEN-002` (Medium) | `F-SEN-002` (High), `F2-SEN-002` | bond locked, not lost; High defensible only for the registered-sentinel variant |
| `F3-SEN-001` (High) | `F-SEN-008` (ledger Low, rv-sen High) | deterministic launch blocker on the documented configuration |

Facts from the two researcher consults that bear on rows outside PC3's list (no row changed here; flagged for whoever owns them):

- Nethermind 1.39.0 and later answer `eth_getLogs` by a processed but not yet canonical hash with `[]` and success (PR #11906, `LogFinder.cs#L205-L208` at 2.1.0), and PublicNode's Gnosis endpoint ran 1.39.3 on 2026-09-27. The default fetch has no completeness check (`F2-CORE-003`, ledger Informational), so behind a load balancer that answer is accepted on the first try. The handbook's "before 1.36" note (`docs/validator-handbook.md:37`) is stale for this reason. Worth a fresh practicality look for `F2-CORE-003`.
- Erigon v3.7.0 and later (PR #23690, merged 2026-09-01) answer a side-chain hash with "block not found" (-32000), not -32001. If a block's logs are still failing when that block is reorged out, the event watcher retries the orphaned hash forever and the block watcher is never polled: a concrete client for run 2's `F2-CORE-005` (Medium), and the answer to the client data C3-COV said R1's observation O9 needed.
- 1RPC answered rate limits with "-32001 usage limit" on 2026-10-07 (the maintainers' RPC endpoint notes), which the indexer reads as "resource not found" and turns into a revalidation (`F3-XC-160`'s path).
- Gnosis plans to move to the Ethereum Economic Zone with 2 s blocks (GIP-153, genesis targeted December 2026 to January 2027). Every timing-based call above (windows of 6 blocks, the 5 s reorg record, `oracle_timeout` 24) needs a re-check then.
