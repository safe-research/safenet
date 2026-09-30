# In-flight assessment (IF-MISC) — unmerged branches outside the senbat stack

**Assessor:** IF-MISC. **Audit branch:** `audit/rust-services` at `cfabcaa` (its `crates/` equal `origin/main` `8b6a75d`). **Method:** every branch was read with `git diff origin/main...<ref>`, `git show <ref>:<path>` and `git merge-tree --write-tree`; nothing was fetched, checked out, stashed or committed, and no finding file or report was edited. One PoC was executed against a `git archive` extraction of a cherry-picked tree in the scratchpad (see §1.3); the repository's tracked files were verified untouched before and after. Judged against [`RECONCILIATION.md`](../../../report/RECONCILIATION.md) §2 and the finding files; relation vocabulary: **Resolves / Partially resolves / Changes / Worsens or introduces / Unrelated / Stale-superseded**. Every `path:line` is on the named branch unless marked `main`.

## 0. Branch positions

| Branch | Head | ahead / behind `origin/main` | Merge base | Recorded before |
| --- | --- | --- | --- | --- |
| `origin/feat/optimistic_block_transition` (PR #915) | `b2aad06` | 2 / 31 | `199629e` | `b2aad06`, 2 / 31 ([`IN-FLIGHT.md`](../../../report/IN-FLIGHT.md) status table) — **unchanged** |
| `origin/feat/batex_0` (#899) | `07b02ed` | 1 / 31 | `199629e` | "likewise" — **unchanged** |
| `origin/fix/batex_1` (#900) | `413fbb0` | 2 / 31 | `199629e` | "likewise" — **unchanged** |
| `origin/feat/batex_2` (#902) | `125def1` | 3 / 31 | `199629e` | "likewise" — **unchanged** |
| `origin/feat/batex_3` (#903) | `8118b69` | 4 / 31 | `199629e` | "likewise" — **unchanged** |
| `origin/feat/batex_4` (#904) | `d6edbb6` | 5 / 31 | `199629e` | `d6edbb6`, 5 / 31 — **unchanged** |
| `origin/pr/pin-reality-module-test-deps` | `ab85f48` | 1 / 20 | `5cc096e` | not assessed before |
| `origin/pr/reality-veto-module-contract` | `3243b52` | 2 / 20 | `5cc096e` | not assessed before |
| `origin/pr/reality-veto-module-tests` | `8b2fd12` | 5 / 20 | `5cc096e` | not assessed before |
| `origin/pr/reality-veto-module-deploy-and-runbook` | `97d4acf` | 6 / 20 | `5cc096e` | not assessed before |
| `origin/fix/issue_820_exceeding_reorgs` | `33fcdcc` | 1 / 106 | `246c331` | not assessed before |
| `origin/wip/nonce-gen-optimizations` | `abf32d8` | 2 / 189 | `4bf4419` | not assessed before |
| `origin/obs/validator-metrics` | `349f484` | 1 / 88 | `b3b7c9e` | not assessed before |
| `origin/ncs/4` | `37f35ed` | 4 / 165 | `3aa20d2` | not assessed before |
| `origin/wip/safe-tx-types-refactor` | `e566a06` | 1 / 189 | `4bf4419` | not assessed before |
| `origin/wip/open-ended-rules` | `cb4d610` | 6 / 189 | `4bf4419` | not assessed before |

Round 1 recorded only `feat/batex_4`'s hash explicitly; the other four batex heads are recorded here for the first time so the next assessment has something to compare against.

## 1. `origin/feat/optimistic_block_transition` (PR #915)

### 1.1 Position

- **Head unchanged** since Round 2 of `IN-FLIGHT.md`: `b2aad06`, 2 ahead / 31 behind. Its two commits are `dd2c54d` "Adjust deadline handling for quicker reaction" (the unsquashed #914) and `b2aad06` "Optimistically apply pending block transition".
- **Still based on a pre-#914, pre-pruning `main`** (`199629e`, "[Part 7] Use oracle events over local inference (#891)"). `main`'s `crates/sentinel` and `crates/core` differ from `dd2c54d` only by the #914 squash's review delta (`crates/sentinel/src/bindings.rs` +2 `daoFeeShare` lines, one `#[allow(clippy::too_many_arguments)]` in `service.rs`) and the pruning stack's `crates/core/src/driver.rs` / `effects.rs` changes.
- **Rebase is clean.** A three-way merge of `main` and the branch conflicts (both sides rewrite the same `handle_block_advance` lines relative to `199629e`), but the cherry-pick of `b2aad06` alone onto `main` — `git merge-tree --write-tree --merge-base=dd2c54d origin/main b2aad06` — produces tree `f73f6e9` with no conflicts. That tree is what §1.3 executed.
- `main`'s `STOPGAP` note (`crates/sentinel/src/service.rs:398-402` on `main`, deferring the compensation revert to safe-research/safenet#471) is removed by `b2aad06`; the branch is the revert the note promises.

### 1.2 What the branch does (own diff, `b2aad06^..b2aad06`)

`crates/core/src/state/mod.rs` (+258 / −23): `Status::BlockPending` gains `applied: bool` (`:127-130`); the log-range arm commits the snapshot **before** running the next block's transition (`:287`) and, in live indexing, runs `apply_transition(NewBlock(pending))` immediately after the previous block's logs (`:309-321`, `applied: true`); `BlockUpdate::New` skips the transition when `applied` (`:250`); the warp arm restores `snapshots.current()` when the pending block was applied (`:206-223`); warp completion and reorg both leave `applied: false` (`:298-303`, `:232-239`). Four new core tests. `crates/sentinel/src/service.rs` (+85 / −67): every deadline comparison returns to the inclusive form (`:404`, `:408`, `:418`), the `self_committed == false` drop no longer waits a block (`:428`), and the two #914 tests are rewritten (`flow_does_not_reveal_before_the_commit_window_closes`, `flow_reveals_a_commit_that_landed_in_the_deadline_block`).

### 1.3 Relation to the ledger

| Finding | Relation | Evidence | Note |
| --- | --- | --- | --- |
| [`F2-SEN-010`](../../../findings/F2-SEN-010.md) (High 93) | **Resolves the #914-introduced mechanism** | Executed on the cherry-picked tree `f73f6e9`: `poc/F2-SEN-010/poc.rs` pasted into `mod tests` of the scratch copy's `service.rs`. Unadapted, all three tests fail on the PoC's own #914 precondition (`"precondition (the delta's own test): Reveal at NewBlock(20)"`) — #915 moves that reveal to `NewBlock(21)` by design. With the precondition adapted to #915's delivery order (`NewBlock(20)` → no reveal; `Committed(OTHER)@20`; `NewBlock(21)` → reveal; two lines changed in the scratch copy only), **3/3 pass**: `after Committed(OTHER)@20 … CollectingVotes { committed_count: 2 …}`, `Revealed(self)@21 → []` (no premature `Finalize`), `Revealed(OTHER)@21 … tracked=true`. The copy's full `sentinel` (47) and `safenet-core` (102) suites pass. | The deadline-block commit is tallied because the switch now runs after block `commitDeadline`'s logs (`state/mod.rs:309-321`) and the comparison is inclusive (`service.rs:418`). The adversarial form of `F2-SEN-010` (a sentinel committing in block `commitDeadline` and never revealing) also loses its lever: the local count is exact, so honest sentinels wait for `reveal_deadline` and finalize by timeout. **Not** resolved: [`F-SEN-002`](../../../findings/F-SEN-002.md) (commits before the verdict are still untallied) and run 1's A4 trigger (a `Committed` lost to an incomplete `eth_getLogs`); remediation convergence item 4's "stop early-finalizing on local tallies" is still the only fix that closes both. Certainty: E1 at the transition level; the core delivery order is covered by the branch's own `pending_block_transition_runs_before_the_block_is_observed` test. |
| [`F-SEN-016`](../../../findings/F-SEN-016.md) D2 (warp rollback discards effect results) | **Still present, latent** | `crates/core/src/state/mod.rs:206-223` replaces live state with `snapshots.current()` when `applied: true`; resumes are applied without a commit (`resume_updates_live_state_without_committing_a_snapshot`). | The state machine starts with `applied: false` (`:165-171`) and `Warp` is emitted only at startup, so the rollback branch is unreachable today; it becomes live the day a warp is emitted mid-run. Same family as `F2-CORE-030`. |
| `F-SEN-016` D3 (early-transition actions re-queued after a reorg) | **Still present** | Reorg arm `:232-239` sets `applied: false`; the replacement block re-runs the transition (`:246-253`, test `reorg_discards_an_optimistically_applied_pending_block`); actions already enqueued for the uncled pending block are not recalled (the doc comment at `:97-100` says so). | Closed by the `F-CORE-067` idempotency key (remediation convergence item 6), not by anything in #915. |
| `F-SEN-016` D4 (validator keygen deadlines one block earlier) | **Restated: no contract mismatch to violate** | `main`: `crates/validator/src/state/keygen.rs:1010`, `:1033`, `:1053`, `:1083-1087` (`block >= *deadline`), `sign.rs:585-706` (`*deadline <= block`). `contracts/src` has no `block.number` deadline for key generation or signing (`Consensus.sol:380,396` compare only `rolloverBlock`; `FROSTCoordinator.sol` none). | These deadlines are local. Under #915 the `block == deadline` arm evaluates over the same log set as on `main` (blocks `<= deadline − 1` in both orderings), one block earlier in wall-clock; the exclusion/restart actions land one block earlier. A mixed-version fleet decides identically from identical information. Residual: unexecuted (static, ~80 %); the re-audit D4 asked for reduces to confirming there is no onchain window the validator must not act inside. |
| [`F2-SEN-003`](../../../findings/F2-SEN-003.md) (Medium 90) | **Unrelated** | `finalize()` (`service.rs:631-671` on `main`) is untouched; warps still deliver no `NewBlock`; warp completion leaves `applied: false` (`state/mod.rs:298-303`), so the restart route in the finding is unchanged. | — |
| [`F2-CORE-030`](../../../findings/F2-CORE-030.md) / [`F-CORE-031`](../../../findings/F-CORE-031.md) (High 92) | **Changes** (as Round 2 said) | Commit before transition (`:287`): effects emitted by the pending block's transition are no longer in the snapshot, so a restart re-runs the transition and re-emits them; log-originated effects are still lost; D2 adds a latent rollback point. | Not a fix; the durable pending-effect set (remediation item 1) is still required. |
| [`F-CORE-067`](../../../findings/F-CORE-067.md) (Medium 98) | **Unchanged in kind, one more block of replay** | Every restart re-applies the pending block's transition (its snapshot precedes it, `:287`), every reorg below it re-applies it (D3); both re-enqueue that block's actions without a key. | — |
| Remediation convergence item 5 (synthetic `NewBlock` at the end of each warp page) | **Design interaction, not a conflict** | `state/mod.rs:206-223`, `:298-303`. | A synthetic `NewBlock(to)` inside the warp arm must set `applied` consistently with `:298-303`, otherwise the following `BlockUpdate::New` double-applies. Item 5's sentence "the validator needs the same `<`/`<=` check the sentinel got in #914" is **moot once #915 lands**: all comparisons return to inclusive and the shift lives in core. |
| `F2-SEN-010` remediation option 1 (`poc/F2-SEN-010/remediation.patch`, counts a `Committed` seen in `CollectingVotes`) | **Composes, does not conflict** | Patch hunk at `service.rs:304-312`. | Under #915 no live `Committed` reaches `CollectingVotes`, so the added branch becomes unreachable in live indexing and its comment's premise ("the switch runs before that block's logs") becomes false. Apply one of the two; if both, rewrite the comment. |

**Verdict for #915:** unchanged head; rebases cleanly onto `main`; **resolves `F2-SEN-010`'s mechanism** (executed); leaves `F-SEN-002`, D2 (latent), D3 and the rest of the ledger as Round 2 recorded; D4 shrinks to a timing note. Re-run list from `IN-FLIGHT.md` Round 2 still applies on the rebased tip (`poc/F2-SEN-001`…`010`, `F-SEN-001`, `F-SEN-002`, `F-SEN-015`, `F-CORE-067`, `F-CORE-001`, `poc/F-VAL-005-066` ordering case); add a D3 test (uncle below an applied pending block, assert the action count) before merge.

## 2. Batched Execution stack (`feat/batex_0` … `feat/batex_4`, PRs #899–#904)

Heads confirmed unchanged (§0 table: `07b02ed`, `413fbb0`, `125def1`, `8118b69`, `d6edbb6`; every one 31 behind, merge base `199629e`); not re-reviewed. **`F-CORE-068` and `F-CORE-069` status:** both remain **forward-looking, not in the tree at `fe9e84c`/`cfabcaa`** (`RECONCILIATION.md` §2.1 rows, §3, §7). Nothing on any pushed branch changed since Round 1 filed them: Phases 5–10 of the epic are still absent, so `Transaction::authorization` is still never set and Phases 3–4 remain behaviourally inert; the defects (`Safenet7702Executor.sol:107-123` guard-and-swallow with a nonce-only `mark_executed`; the span-aware allocation at `crates/core/src/tx/storage.rs:190-215` with the self-clearing single `error!` at `tx/mod.rs:198-222` and the unordered `pending_delegation` query at `storage.rs:126-137`, all on `feat/batex_4`) are exactly as filed. The stack still needs a rebase over the pruning merges and #914 (31 commits; it touches `crates/core/src/tx/*`, `crates/validator/src/service/action.rs`, `crates/sentinel/src/service.rs`, none of which the pruning stack rewrote, so a conflict-free rebase is likely but was not attempted). Round 1's re-run list (`IN-FLIGHT.md` "What to re-run when this merges", items 1–12) stands unchanged.

## 3. Reality Veto branches

| Branch | Files under `crates/` | Files under `contracts/src` | Other |
| --- | --- | --- | --- |
| `pr/pin-reality-module-test-deps` (`ab85f48`) | none | none | `.gitmodules`, `contracts/foundry.toml` (remappings + one `fs_permissions` read), four `contracts/lib/*` submodules |
| `pr/reality-veto-module-contract` (`3243b52`) | none | `interfaces/IRealityModule.sol` (new), `veto/RealityVetoModule.sol` (new), `veto/README.md` (new) | as above |
| `pr/reality-veto-module-tests` (`8b2fd12`) | none | same three (new) | plus `contracts/test/RealityVetoModule.t.sol`, `test/util/MockRealitio.sol`, `test/util/RealityModuleDeployer.sol` |
| `pr/reality-veto-module-deploy-and-runbook` (`97d4acf`) | none | same three (new) | plus `contracts/script/DeployRealityVetoModule.s.sol`, `contracts/script/README.md`, `contracts/.env.sample`, `Justfile` |

- **No in-scope crate code changes** on any of the four; the "3 files" are the three new files under `contracts/src`, all additions — no existing contract, library or interface is modified.
- **No Rust binding is affected.** The services' bindings are hand-written inline `sol!` declarations (`crates/sentinel/src/bindings.rs:7,77,142`, `crates/validator/src/bindings.rs:15`, `crates/core/src/index/events.rs`), not generated from Foundry artifacts, and the `foundry.toml` change adds test-only remappings ("Nothing in `src/` uses any of them", `foundry.toml:24-34` on the branch) and a read permission for the module's own artifact. No finding's ABI assumption (`SentinelOracleRequests.sol:117/126/168-172` for `F-SEN-002`/`F2-SEN-010`/`F2-SEN-003`, `FROSTNonceCommitmentSet.sol:91-105` for `F-VAL-030`/`F-VAL-039`, `SentinelOracle.sol:62-68` for `F-SEN-008`) is touched.
- Round 2's reading of the #917 epic holds for the implementation: `to`, `value`, `operation` and selector are fixed in code (`RealityVetoModule.sol:39-40`, `Enum.Operation.Call`, `abi.encodeCall(IRealityModule.markProposalAsInvalid, …)`), success is required (`VetoFailed()`), and the question hash is computed onchain from `buildQuestion` (`:36-37`). Round 2's gap also holds: the runbook's enable step (`contracts/script/README.md:236`, `veto/README.md:54`) still does not ask whether the SafeDAO Safe is Safenet-protected, in which case a signed `enableModule` is the Charter R-4.1 case. Out of the Rust audit's scope; recorded for completeness.

## 4. Older work-in-progress branches touching in-scope crates

### 4.1 `origin/fix/issue_820_exceeding_reorgs` (`33fcdcc`)

- **Effectively merged, in reworked form — Stale-superseded.** `main`'s `40467c5` "Exit on reorgs exceeding maximum reorg depth (#834)" touches the same six files; `crates/core/src/driver.rs` and `crates/core/src/index/mod.rs` are byte-identical between the branch and the squash; `crates/core/src/index/blocks.rs` was reworked in review (the anchor moved out of `recent` into a separate `SafeBlock` field, `main` `blocks.rs:163-172`, `:435-439`, `:453-462`).
- **One hunk the review dropped is the loud failure `F-CORE-005`/`F2-CORE-008` are about.** On the branch `revalidate_last_block` returns `Error::ExceededMaxReorgDepth` when the block to invalidate is the anchor itself (`blocks.rs:504-506`, `if last_index == 0`), with a test at `max_reorg_depth = 0` (`:1166` `fails_loudly_when_revalidation_invalidates_the_anchor`, asserting `Err(ExceededMaxReorgDepth(0))`); the driver already exits on that error class (`driver.rs:185`, `:209`, identical on `main`). The merged design keeps the anchor outside `recent`, so with depth 0 `recent` is always empty, `rposition` finds nothing and the `-32001` path returns `Ok(None)` and spins (`main` `blocks.rs:494-501`) — exactly [`F-CORE-005`](../../../findings/F-CORE-005.md) / [`F2-CORE-008`](../../../findings/F2-CORE-008.md) (Low 75, Confirmed). Relation: **Partially resolves `F-CORE-005`/`F2-CORE-008` in a superseded design**; the fix is a port, not a merge — when `recent` is empty and a revalidation is requested, compare `safe.hash` against the node and return `ExceededMaxReorgDepth`.
- [`F2-CORE-011`](../../../findings/F2-CORE-011.md) (no anchor persisted on a fresh start) and [`F-CORE-001`](../../../findings/F-CORE-001.md) (reorg-depth protection not persisted): **Unrelated** — neither the branch nor #834 persists the anchor; both keep it in memory.
- No conflict with a recommended remediation (item 9 asks for persistence, which this never had).

### 4.2 `origin/wip/nonce-gen-optimizations` (`abf32d8`)

- **Stale-superseded.** Base `4bf4419` (189 behind); a prototype of the Nonces series that landed as #718, #724 and [Nonces 2]–[7b] (#745–#773): the branch's `crates/validator/src/service/nonce_generator.rs` became `secrets/nonces.rs`, its `NonceState`/`NonceIndex` in `state/preprocess.rs` became `main`'s `NonceState` at `state/preprocess.rs:185-252`. Not mergeable as is (`secrets.rs` was split into `secrets/{mod,nonces,store}.rs`).
- **One design difference matters for [`F2-VAL-030`](../../../findings/F2-VAL-030.md) (High 97, with `F-VAL-030`).** On the branch `link` clears every pending reservation before recording the onchain assignment (`state/preprocess.rs:50-51`, `self.chunks.retain(|_, root| root.is_some())`) and `reserve_chunk` refuses while any reservation is pending (`:38-39`); on `main` `link` is a plain insert (`preprocess.rs:211-213`) and `expected_chunk` counts the dangling `None` — `F2-VAL-030`'s consequence 2, the `expected_chunk` cascade. That is the second half of `F2-VAL-030` remediation option 2 ("on `handle_preprocess` drop any `None` entry whose index differs from `event.chunk`"). Relation: **Partially resolves `F2-VAL-030` (consequence 2) in a superseded design.** Consequence 1 is not addressed: the branch's `available()` (`:76-88`) counts pending reservations exactly as `main`'s does.
- [`F-VAL-031`](../../../findings/F-VAL-031.md) (dead worker never detected): **Changes, superseded** — the branch models `Worker::Failed | Worker::Stopped` and surfaces it as `Error::Unavailable` (`service/nonce_generator.rs:88-95`, `:155-162`); no restart, no log, and the model did not survive into `secrets/nonces.rs`.
- [`F2-VAL-031`](../../../findings/F2-VAL-031.md), [`F2-VAL-035`](../../../findings/F2-VAL-035.md), [`F-VAL-039`](../../../findings/F-VAL-039.md), [`F-VAL-066`](../../../findings/F-VAL-066.md): **Unrelated** — the branch predates group retention, reconciliation and pruning (Nonces 4b–4d, #754–#757, and the pruning stack); the top-up threshold is the same `NONCE_TOPUP_THRESHOLD = 100` (`:12`).

### 4.3 `origin/obs/validator-metrics` (`349f484`)

- **Stale-superseded** by the merged Metrics series (#870, #873, #874, #877, #878, #879, #880; base `b3b7c9e` = #836). The branch's `Metrics` trait with `NoopMetrics`, `ProcessingCursor` and `RpcObserver` in `crates/core` and its `validator_block_number` / `validator_effects` / `validator_event_index` / `validator_reorgs` / `validator_rpc_requests` / `validator_transitions` gauges (`crates/validator/src/metrics.rs`) exist on `main` as `metrics`-crate macros with the `safenet_core_block_number` (`status` = seen/processed), `safenet_core_rpc_requests_total`, `safenet_core_uncled_blocks_total`, `safenet_validator_effects_total`, `safenet_validator_transitions_total`, `safenet_validator_secrets_total`, `safenet_validator_housekeeping_total` names — a superset.
- [`F2-CORE-067`](../../../findings/F2-CORE-067.md) (queue exports no metrics, hash never persisted): **Unrelated** — the branch touches nothing under `crates/core/src/tx/` (`git diff --stat -- crates/core/src/tx` is empty). [`F2-VAL-066`](../../../findings/F2-VAL-066.md) / [`F-VAL-064`](../../../findings/F-VAL-064.md) (`/health` constant, exit 0): **Unrelated** — no health or exit-status change. Nothing to salvage.

### 4.4 `origin/ncs/4` (`37f35ed`)

- **Two of four commits merged verbatim** — `8f0079f` "[Nonces 2]" and `32d3dc1` "[Nonces 3]" are `main`'s #745 and #746; the two `wip` commits (`39d4839`, `37f35ed`: `secrets/store.rs` +118 retaining groups, `state/keygen.rs` refactor, `state/preprocess.rs` +72) are the drafts of [Nonces 4a]/[4b] (#753/#754). **Stale-superseded**, base 165 behind. Its files at the tip differ from `main`'s [Nonces 3] merge only by those drafts. **Unrelated** to every ledger row (the finding-relevant code — reservation accounting, reconciliation, pruning — postdates it).

### 4.5 `origin/wip/safe-tx-types-refactor` (`e566a06`)

**Stale-superseded, engine scope:** a one-commit refactor of `crates/safe-tx` (removed from `main` by #829) with mechanical `bindings.rs`/`hashing.rs` fallout in sentinel and validator; no relation to any ledger row.

### 4.6 `origin/wip/open-ended-rules` (`cb4d610`)

**Stale-superseded, engine scope:** five of its six commits are `main`'s [Sentinel Engine 1a–1e] (#712–#716); the unmerged tip `cb4d610 wip` touches `crates/safe-tx` and the pre-split sentinel checkers, both gone from `main`; no relation to any ledger row.

## 5. Consolidated table

| Branch / PR | Finding | Relation | Evidence (branch `path:line`) | Action needed |
| --- | --- | --- | --- | --- |
| `feat/optimistic_block_transition` #915 | `F2-SEN-010` | Resolves (mechanism) | `crates/sentinel/src/service.rs:418`, `crates/core/src/state/mod.rs:309-321`; adapted `poc/F2-SEN-010/poc.rs` 3/3 pass on cherry-pick tree `f73f6e9` | Rebase (clean) and merge; then re-run `poc/F2-SEN-010` unadapted-minus-precondition and `poc/F2-SEN-002` to confirm `F-SEN-002` is untouched |
| #915 | `F-SEN-002` | Unrelated | `service.rs:372-375` on `main`, unchanged | Remediation item 4 still required |
| #915 | `F-SEN-016` D2 | Still present (latent) | `state/mod.rs:206-223` | Snapshot after resumes or re-apply them on warp rollback; add a test before any mid-run `Warp` is introduced |
| #915 | `F-SEN-016` D3 | Still present | `state/mod.rs:232-239`, `:246-253` | Closed by the `F-CORE-067` idempotency key; add a duplicate-action test on uncle-below-applied-pending |
| #915 | `F-SEN-016` D4 | Restated: timing only | `main` `keygen.rs:1010-1087`, `sign.rs:585-706`; no onchain deadline in `contracts/src` | Note in the PR that keygen/sign deadlines are local; no code change |
| #915 | `F2-SEN-003` | Unrelated | `service.rs:631-671` on `main` untouched | — |
| #915 | `F2-CORE-030` / `F-CORE-031` | Changes | `state/mod.rs:287` | Remediation item 1 still required |
| #915 | `F-CORE-067` | Unchanged in kind | `state/mod.rs:287`, `:232-239` | Remediation item 6 |
| #915 | remediation item 5; `F2-SEN-010` option 1 | Design interaction / composes | `state/mod.rs:298-303`; `remediation.patch` hunk `service.rs:304-312` | Keep `applied` consistent if a synthetic warp `NewBlock` is added; pick #915 or option 1, not both unexamined |
| `feat/batex_0`…`_4` #899–#904 | `F-CORE-068`, `F-CORE-069` | Forward-looking, unchanged | `feat/batex_4`: `contracts/src/Safenet7702Executor.sol:107-123`, `crates/core/src/tx/storage.rs:126-137`, `:190-215`, `tx/mod.rs:198-222` | Round 1 re-run list on merge; rebase over 31 commits |
| `pr/reality-veto-*` (4) | all | Unrelated | no `crates/` files; `contracts/src/{interfaces/IRealityModule.sol,veto/RealityVetoModule.sol,veto/README.md}` new | None for the Rust audit; Round 2's Charter R-4.1 question for #917 still open |
| `fix/issue_820_exceeding_reorgs` | `F-CORE-005` / `F2-CORE-008` | Partially resolves (superseded design) | `crates/core/src/index/blocks.rs:504-506`, test `:1166`; dropped by `main` `40467c5` | Port the anchor-invalidation guard to the `SafeBlock` design (`main` `blocks.rs:494-501`) |
| `fix/issue_820_exceeding_reorgs` | `F2-CORE-011`, `F-CORE-001` | Unrelated | anchor never persisted on either side | Remediation item 9 |
| `wip/nonce-gen-optimizations` | `F2-VAL-030` (consequence 2) | Partially resolves (superseded design) | `crates/validator/src/state/preprocess.rs:38-39`, `:50-51` vs `main` `preprocess.rs:211-213` | Adopt `link`'s clear-pending semantics as part of `F2-VAL-030` option 2; consequence 1 still needs option 1 |
| `wip/nonce-gen-optimizations` | `F-VAL-031` | Changes (superseded) | `crates/validator/src/service/nonce_generator.rs:88-95`, `:155-162` | None; the model is gone |
| `wip/nonce-gen-optimizations` | `F2-VAL-031`, `F2-VAL-035`, `F-VAL-039`, `F-VAL-066` | Unrelated | predates the code they cite | — |
| `obs/validator-metrics` | `F2-CORE-067`, `F2-VAL-066`, `F-VAL-064` | Unrelated / Stale-superseded | no `crates/core/src/tx/` change; superseded by #870–#880 | Delete or close the branch |
| `ncs/4` | all | Stale-superseded | 2 of 4 commits = #745/#746; wips = drafts of #753/#754 | Delete or close the branch |
| `wip/safe-tx-types-refactor`, `wip/open-ended-rules` | all | Stale-superseded (engine scope) | `crates/safe-tx` removed by #829; 1a–1e merged as #712–#716 | Delete or close the branches |

## 6. Actions for the team

1. **#915:** rebase `b2aad06` onto `main` (the cherry-pick is conflict-free) and merge it ahead of any standalone `F2-SEN-010` patch — it removes the #914-introduced undercount at its root; keep the `F-SEN-002` fix (stop early-finalizing on local tallies) on the plan, because #915 does not touch it. Before merge: add a D3 duplicate-action test, and decide whether a mid-run `Warp` can ever occur (if yes, D2 becomes live).
2. **`F-CORE-005`/`F2-CORE-008`:** port the `revalidate_last_block` anchor-invalidation guard from `fix/issue_820_exceeding_reorgs` (`blocks.rs:504-506`) into the `SafeBlock` design on `main`; it is the loud failure the depth-0 documentation promises and the review of #834 dropped it.
3. **`F2-VAL-030`:** when implementing option 2, take `link`'s "clear every pending reservation" semantics from `wip/nonce-gen-optimizations` (`preprocess.rs:50-51`) rather than re-deriving them; option 1 (an explicit failure resume) is still needed for consequence 1.
4. **Housekeeping:** `obs/validator-metrics`, `ncs/4`, `wip/nonce-gen-optimizations`, `wip/safe-tx-types-refactor`, `wip/open-ended-rules` and `fix/issue_820_exceeding_reorgs` are all superseded by merged work and 88–189 commits behind; closing them removes the risk of an accidental revival of the pre-review designs (in particular the pre-`SafeBlock` watcher and the pre-Nonces-5 reservation model).
5. **Reality Veto:** no Rust-side action; the Round 2 question on #917 (is the SafeDAO Safe Safenet-protected, making `enableModule` a Charter R-4.1 case) should be answered in the runbook before the deploy branch merges.
6. **Batex:** unchanged; Round 1's twelve-item re-run list applies when #904 merges, after a rebase over the pruning merges and #914.

## 7. Execution record

- Cherry-pick tree: `git merge-tree --write-tree --merge-base=dd2c54d origin/main b2aad06` → `f73f6e9`, extracted with `git archive` to the scratchpad (`pr915-on-main/`); `CARGO_TARGET_DIR=~/.cache/safenet-run2/target-misc`, `CARGO_BUILD_JOBS=2`, `cargo test --offline`.
- `cargo test -p sentinel --bins qa2_sen_010 -- --nocapture --test-threads=1`: unadapted PoC 0/3 (precondition assertion `service.rs:2399` in the copy); adapted PoC 3/3. `cargo test -p sentinel -p safenet-core`: 47 + 102 pass.
- The adaptation (scratch copy only): the precondition `assert!(has(&c20, is_reveal), …)` became `assert!(!has(&c20, is_reveal), …)` and, after `Committed(OTHER)@20`, `let (state, c21) = svc.apply_transition(state, Message::NewBlock(COMMIT_DEADLINE + 1)); assert!(has(&c21, is_reveal), …)`. The tracked `rust-audit/poc/F2-SEN-010/poc.rs` was not modified.
- `git status` on the repository showed no tracked change before or after; the only untracked entries are this directory's files.
