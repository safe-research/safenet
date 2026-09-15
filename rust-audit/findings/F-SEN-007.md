# F-SEN-007 No balance, allowance, registration or chain pre-check: a sentinel that cannot possibly commit still pays for an `approve` and a reverting `commit` on every single request, silently and forever

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, service.rs / main.rs |
| Location | crates/sentinel/src/service.rs:226-242, 676-704 (related: crates/sentinel/src/main.rs:64-83, crates/core/src/tx/mod.rs:241-296) |
| Severity | Medium / Low |
| Certainty | 80% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A1, A2 |
| Tags | config, dos |

## Claim

`commit_vote` emits `ApproveToken` + `Commit` for every request whose engine check returns a verdict, with no check that the commit can succeed. Nothing in the sentinel or in the transaction queue verifies, at startup or per request, that:

- the signer is a registered and active sentinel (`SentinelOracle.commit` requires `$sentinelMap.isActive(msg.sender)`, `contracts/src/SentinelOracle.sol:231-232`);
- the signer holds at least `bondTarget` of the fee token (`commit` ends in `safeTransferFrom(msg.sender, …, bondAmount)`, `contracts/src/SentinelOracle.sol:236`);
- the configured `fee_token` is actually the oracle's `FEE_TOKEN`;
- the configured `consensus` is the oracle's `PROPOSER`, without which every locally computed `requestId` fails to match any `NewRequest`.

`TransactionQueue::submit_transaction` also performs no `eth_call` or gas estimation before broadcasting (`core/tx/mod.rs:241-296`), so nothing catches the failure earlier either. The result of any of these misconfigurations, or simply of the fee-token balance running out, is a steady per-request burn: the `approve` succeeds (55,000 gas budget) and the `commit` reverts (250,000 gas budget), forever, with no error visible beyond the absence of `Committed` events — the sentinel's own logs report the actions as submitted, and the queue never inspects execution status (`mark_executed` only compares nonces, `core/tx/storage.rs:222-232`).

The `consensus`-misconfiguration variant is quieter still and costs nothing but total non-participation: request ids never match, so `handle_new_request` logs `"ignoring new request for an untracked proposal"` at **`debug`** level (`service.rs:286-289`) and the sentinel silently never votes on anything.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The bond and commit are emitted with no precondition beyond having a verdict | E2 | crates/sentinel/src/service.rs:206-226 | `        let Request {`<br>`            bond_target,`<br>`            slash_amount,`<br>`            commit_deadline,`<br>`            reveal_deadline,`<br>`        } = request;`<br>`        let salt = self.signer.reveal_salt(request_id);`<br>`        let hash = commit_hash(self.signer.address, request_id, approve, salt, &reason);`<br>`        state.0.insert(`<br>`            request_id,`<br>`            RequestState::CollectingCommitments {`<br>`                approve,`<br>`                reason,`<br>`                slash_amount,`<br>`                commit_deadline,`<br>`                reveal_deadline,`<br>`                committed_count: 0,`<br>`                self_committed: false,`<br>`            },`<br>`        );` |
| 2 | The approve is encoded against the configured `fee_token` with a fixed gas budget | E2 | crates/sentinel/src/service.rs:678-688 | `            SentinelActionKind::ApproveToken { bond } => Transaction {`<br>`                to: self.fee_token,`<br>`                value: U256::ZERO,`<br>`                data: ERC20::approveCall {`<br>`                    spender: self.oracle,`<br>`                    amount: bond,`<br>`                }`<br>`                .abi_encode`<br>`                .into,`<br>`                gas: 55_000,`<br>`            },` |
| 3 | Startup wires the addresses through with no onchain validation of any of them | E2 | crates/sentinel/src/main.rs:64-73 | `    let service = SentinelService::new(`<br>`        config.oracle,`<br>`        config.sentinel.fee_token,`<br>`        config.consensus,`<br>`        config.signer.clone,`<br>`        U256::from(chain_id),`<br>`        config.sentinel.voting_window,`<br>`        EngineClient::new(config.sentinel.engine)?,`<br>`        engine_timeout,`<br>`    );` |
| 4 | Nothing simulates or estimates before broadcasting | E2 | crates/core/src/tx/mod.rs:246-265 | `        let chain_id = self.provider.chain_id;`<br>`        let fees = self.fees.await?;`<br>`        let transaction = transaction.build(chain_id, fees);`<br>`        let submission = Submission {`<br>`            block: Some(block),`<br>`            nonce: transaction.nonce,`<br>`            fees: Eip1559Estimation {`<br>`                max_fee_per_gas: transaction.max_fee_per_gas,`<br>`                max_priority_fee_per_gas: transaction.max_priority_fee_per_gas,`<br>`            },`<br>`        };`<br>``<br>`        let signed = self.signer.sign_transaction(transaction)?;` |
| 5 | An unregistered or deactivated sentinel's `commit` always reverts | E2 (Solidity reference, A7) | contracts/src/SentinelOracle.sol:231-236 | `    function commit(bytes32 requestId, bytes32 commitHash) external {`<br>`        require($sentinelMap.isActive(msg.sender), SentinelNotActive);`<br>`        SentinelOracleRequest.T storage request = $requests.get(requestId);`<br>`        uint96 bondAmount = request.applyCommit;`<br>`        $commitments.add(requestId, msg.sender, commitHash, bondAmount);`<br>`        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), bondAmount);`<br>`    }` |
| 6 | A wrong `consensus` (or `chain_id`) makes every request id mismatch, and that is reported only at `debug` | E2 | crates/sentinel/src/service.rs:286-289 | `            None => {`<br>`                tracing::debug!(%request_id, "ignoring new request for an untracked proposal");`<br>`                (state, Vec::new)`<br>`            }` |
| 7 | The request id is derived entirely from configured values plus the event | E2 | crates/sentinel/src/service.rs:108-115 | `        let request_id = oracle_tx_proposal_hash(`<br>`            self.chain_id,`<br>`            self.consensus,`<br>`            event.epoch,`<br>`            event.oracle,`<br>`            event.oracleData.clone,`<br>`            event.safeTxHash,`<br>`        );` |

## Trigger

Three concrete, non-adversarial cases, plus one adversarial amplification:

1. **Out of fee token.** A sentinel that has been running normally has its fee-token balance drawn down by bonds (each locked for `COMMIT_WINDOW + REVEAL_WINDOW` blocks) or by lost disputes. From the first request it cannot fund, every subsequent request costs an `approve` plus a reverting `commit`, indefinitely. No metric or log distinguishes this from normal operation — `requests_participated_total` simply stops rising, because it is only incremented on the `Committed(self)` event (`service.rs:325-329`).
2. **Not yet activated.** Governance calls `addSentinel`, which schedules activation after `GOVERNANCE_DELAY` (`contracts/src/SentinelOracle.sol:349-351`, `contracts/src/libraries/SentinelMap.sol:48-51`). An operator who starts the binary immediately burns gas on every request until the delay elapses.
3. **Wrong `consensus` address in the config.** All request ids mismatch, `handle_new_request` drops every `NewRequest` at `debug` level (basis 6), and the sentinel never votes — while still spending an engine check per proposal. Nothing fails loudly, and the `[sentinel]` block's `TODO` (`config.rs:44-48`) explicitly relies on "fails loudly" only for _missing_ values, not wrong ones.
4. **Adversarial amplification (A2).** Because proposals are permissionless (any sponsor may call `Consensus.proposeTransaction`), an attacker who observes a sentinel in state 1 or 2 — visible onchain as an address that emits `approve` but never `Committed` — can spam cheap proposals to drain that sentinel's native-gas balance; combined with the fee refund on a total timeout (F-SEN-004 basis 8), the attacker's own cost is close to proposal gas alone.

## Considered and rejected

- **"The `expires_at` on the actions limits the waste."** It limits how long a doomed action stays queued, not whether it is created; both actions are created for every request.
- **"A revert is cheap."** The `approve` in case 1 and 2 _succeeds_, at full cost. The `commit` reverts, but a revert still pays intrinsic gas plus everything executed up to the failing `require`, which for case 1 includes the ERC-20 `transferFrom` path.
- **"An operator would notice."** They would eventually, from the absence of `Committed` events, but no log line or metric marks the condition: `submit_transaction` logs a successful broadcast at `debug` (`core/tx/mod.rs:259-264`) and the queue never learns that the transaction reverted (basis 4, and `core/tx/storage.rs:222-232` marks executed purely by nonce). `safenet_sentinel_requests_participated_total` (`metrics.rs:60-65`) is a counter with no matching "attempted" counter to compare against.
- **"The handbook covers it."** `docs/sentinel-handbook.md` is reference-only material and, per the prior analysis, says only that "logs will show" — which basis 4 and 6 contradict for these cases.
- **"Startup validation would need the ABI."** It would need three view functions the crate does not currently bind (`FEE_TOKEN`, `PROPOSER`, `sentinelActiveAt`/`isActive`), plus `ERC20.balanceOf`; `ERC20.allowance` is already bound and unused (`bindings.rs:63`), so the pattern exists.
- **False positive check — is there any validation anywhere?** `Config::load` only parses TOML (`config.rs:61-67`); `main.rs:37-83` performs exactly one onchain read, `eth_chainId` inside `Provider::connect`, and it is used to build the EIP-712 domain rather than to check anything.

## Remediation options

1. **Fail fast at startup.** After `Provider::connect`, read `oracle.FEE_TOKEN`, `oracle.PROPOSER` and `oracle.sentinelActiveAt(signer.address)` (adding the three bindings) and refuse to start — or start in a loud degraded mode — when `fee_token != FEE_TOKEN`, `consensus != PROPOSER`, or the signer is not scheduled to activate. Cheap (three `eth_call`s once) and turns three silent failure modes into a startup error. Tradeoff: the sentinel can no longer be started before governance activates it, so a "warn and keep retrying" variant may be preferable.
2. **Per-request affordability check.** Before emitting `ApproveToken` + `Commit`, verify via an effect that `feeToken.balanceOf(self) >= bondTarget` and that the signer is active; skip the request (with a dedicated metric) otherwise. Tradeoff: one RPC read per request, and a race with concurrent bonds; a cached balance updated per block would be enough in practice.
3. **Simulate in the queue.** `eth_call` each transaction at the latest block before broadcasting and drop it on revert, with a counter. Fixes this, F-SEN-006's duplicates, and any future doomed action in one place. Tradeoff: one extra RPC round trip per submission, and a benign race where a simulation passes and inclusion still reverts.
4. **Observability floor.** At minimum, add a `safenet_sentinel_commit_attempts_total` counter next to `requests_participated_total` so the ratio exposes the condition, and raise the `"ignoring new request for an untracked proposal"` log to `warn` when it fires for an oracle address the sentinel is configured to watch.

Tests to add: a `main`-level test (or a `SentinelService::validate` unit) asserting a mismatched `fee_token` is rejected. An integration-script scenario running a sentinel with zero fee-token balance and counting reverted `commit`s.

## Trail

- Reviewer R7: drafted from lead SEN-H5 (extended with the `consensus`/`fee_token` cross-validation half of SEN-H12), self-estimate 80%. All seven basis citations re-opened in this checkout.

## Critic (C-SEN)

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `service.rs:206-226` verbatim; `commit_vote` has no precondition beyond a verdict. |
| 2 | **Supported** | `service.rs:678-688` verbatim; `to: self.fee_token`, `gas: 55_000`, no estimation. |
| 3 | **Supported** | `main.rs:64-73` verbatim. I read `main.rs` in full: between `Config::load` and `driver.run` there is no onchain read of any kind other than `provider.chain_id` (`:43`). |
| 4 | **Supported** | `core/tx/mod.rs:246-265` verbatim; build → sign → send, with no simulation. |
| 5 | **Supported** | `SentinelOracle.sol:231-236` verbatim; `require($sentinelMap.isActive(msg.sender), SentinelNotActive)` then `safeTransferFrom(msg.sender, ...)`. |
| 6 | **Supported** | `service.rs:286-289` verbatim; `tracing::debug!` for an untracked proposal. |
| 7 | **Supported** | `service.rs:108-115` verbatim; `chain_id` and `consensus` are both configured inputs to the request-id derivation. |

Every citation is accurate and the absences are real. No claim marked `H`.

### Severity re-judged: Medium → **Low**

I am lowering this, and the reason is A1 rather than any defect in the analysis.

- The `fee_token`, `consensus`, `oracle` and chain-id variants are all **operator provisioning mistakes**. A1 states that config files are provisioned by an honest operator; under that assumption a wrong address is an operational error whose remedy is a startup assertion, which is precisely "a configuration weakness with limited impact" — Section 8's Low.
- The `isActive` variant is the same shape: governance registration is an operator/deployment precondition, not something an attacker controls.
- The genuinely non-operator variant is **balance exhaustion**, and that is not an independent defect: it is the _downstream symptom_ of F-SEN-002 (bonds never reclaimed) and F-SEN-004 (unbounded outstanding bonds). Counting it at Medium here double-counts impact already scored there.

What survives at full strength, and is the part the report should lead with, is the **observability** claim: the failure is invisible in every variant. `mark_executed` compares nonces only (`core/tx/storage.rs:222-232`), so a reverted `commit` is indistinguishable from a successful one; the `consensus` mismatch logs at `debug`; and `requests_participated_total` (`service.rs:327`) only ever increments on success, so a sentinel that has never participated and one that is silently failing produce identical metric series. There is no counter for "commit submitted but no `Committed` observed".

Under A4 this stays in scope on its own terms — nothing here depends on a malicious RPC; the stale/erroring-RPC case is not what drives it.

### Finding verdict

**Confirmed. Certainty 80%. Severity Medium / Low (corrected).**

`E2` throughout: the mechanism is a set of verified absences, and the trigger (a mis-set address, or an empty fee-token balance) is concrete. Certainty is high; severity is not, and the two should not be conflated.

Recommend the report keep remediation option 1 (startup preflight: read `FEE_TOKEN`, `PROPOSER` and `isActive(signer)` once at boot and refuse to start on mismatch) as the primary fix — it is cheap, one-time, and converts every variant from silent to loud.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 1 in its "warn and keep retrying" variant, plus option 3.**

Option 1 (read `FEE_TOKEN`, `PROPOSER` and `sentinelActiveAt(self)` at startup and refuse to start on a mismatch) is sound and cheap — three `eth_call`s, once — and turns three silent failure modes into a startup error. **Take the variant the option itself flags**: refusing to start when the signer is not yet activated would prevent the normal operational sequence of deploying a sentinel before governance activates it. `fee_token != FEE_TOKEN` and `consensus != PROPOSER` are genuine misconfigurations and should be hard errors; "not yet active" should be a loud degraded mode with a metric, retried each block.

Option 2 (per-request affordability check via an effect) is sound but is the weakest of the three for the cost: one RPC read per request, plus a race with concurrent bonds, to catch a condition that option 1 catches once at startup for free. Its cached-per-block variant is the only form worth building.

Option 3 (`eth_call` before broadcasting, drop on revert) is the strongest and is a core change to `submit_transaction` — the same change as **F-SEN-006 option 2** and **F-SEN-014 option 3**. It fixes this finding, the duplicate-action waste, and the finalize herd in one place, which makes it the best value in the sentinel set. Its cost is one extra RPC round trip per submission on a queue that currently makes at most three calls per block.

Option 4 (a `commit_attempts_total` counter beside `requests_participated_total`, and raising the `"ignoring new request for an untracked proposal"` log to `warn` when it fires for a configured oracle) is the observability floor and should land regardless — as shipped, the ratio that would expose this condition is not computable from any exported metric.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — no pre-check of any kind was added

**Merged-code citations:**

| Cited at `2893917` | Now at | Changed? |
| --- | --- | --- |
| `service.rs:226-242` (`commit_vote` emits `ApproveToken` + `Commit` unconditionally) | `service.rs:226-242` | byte-identical |
| `service.rs:676-704` (encoder) | `service.rs:820-848` | moved only (the three new handlers were inserted above it) |
| `service.rs:286-289` (`debug`-level "ignoring new request for an untracked proposal") | `service.rs:286-289` | byte-identical |
| `crates/sentinel/src/main.rs:64-83` | unchanged | `main.rs` is untouched by the merge |
| `crates/core/src/tx/mod.rs:241-296`, `crates/core/src/tx/storage.rs:222-232` | unchanged | `crates/core` is byte-identical |
| `contracts/src/SentinelOracle.sol:231-236` (`commit`: `isActive` + `safeTransferFrom`) | `contracts/src/SentinelOracle.sol:232-238` (`isActive` at `:233`, `safeTransferFrom` at `:237`) | +1 line only, from the `arbitrationDeadline` insertion at `:261`; the function body is unchanged |

Nothing in the merge verifies registration, balance, fee-token identity or the `consensus` address, and `TransactionQueue::submit_transaction` still performs no `eth_call` or gas estimation. The `consensus`-misconfiguration variant is unchanged, including its `debug` log level.

**Certainty 80% and severity Medium / Low unchanged.** Status left at `Critiqued`.

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against the "Batched Execution" stack (`origin/feat/batex_0` … `origin/feat/batex_4`, PRs #899–#904). **Effect: worsen.** None of the four missing pre-checks is added: `commit_vote` is unchanged apart from `authorization: None` initialisers, `main.rs` gains no startup verification of registration, bond balance, `fee_token` or `consensus`, and `TransactionQueue::submit_transaction` still performs no `eth_call` and no gas estimation before broadcasting — the only change there is the behaviour-preserving reordering of `bumped_fees` ahead of `build`. So the steady per-request burn reproduces verbatim. Batching makes the _diagnosis_ strictly harder, which is the part of this finding that already turns on invisibility. Today the burn at least shows up as a succeeded `approve` followed by a reverted `commit` on the signer's address. Once Phase 7 lands (not on any pushed branch), both calls sit inside one batched self-call: `Safenet7702Executor.execute` swallows the `commit` revert into a `CallFailed(index, result)` event and the batch transaction **succeeds**, so a block explorer shows a healthy transaction, `mark_executed` records it as executed on the nonce advance, and `prune` deletes the row within `max_reorg_depth` blocks. The revert data survives only as a log on the sentinel's own EOA, which the epic confirms nothing watches ("the services do not currently watch their own EOA for logs"). The finding's stated symptom — "no error visible beyond the absence of `Committed` events" — becomes the _only_ symptom. Filed as **`F-CORE-068`**. Severity and certainty unchanged. See `rust-audit/report/IN-FLIGHT.md`.
