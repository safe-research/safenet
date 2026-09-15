# F-SEN-008 Hard-coded gas limits and an unconditional non-zero `approve` assume a plain ERC-20; a proxied, hooked or non-zero-to-non-zero-reverting fee token breaks every commit

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, service.rs |
| Location | crates/sentinel/src/service.rs:676-740 (related: 226-242) |
| Severity | Low / Low |
| Certainty | 52% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A1, A7 |
| Tags | config, input-validation |

## Claim

`SentinelEncoder::encode_action_kind` assigns fixed gas limits — 55,000 for `approve` and 250,000 for `commit`/`reveal`/`finalize`/`claim` — with no estimation and no configuration knob. The 250,000 figure is documented as measured against the reference deployment, but 55,000 for `approve` only fits a plain, unproxied OpenZeppelin-style ERC-20; a transparent/UUPS proxy, a token with transfer hooks, or a token whose `approve` writes extra bookkeeping can exceed it, in which case **every** `approve` runs out of gas and **every** `commit` then reverts for insufficient allowance. The sentinel would burn 55,000 gas per request forever while never participating, and nothing detects it (see F-SEN-007 basis 4 and 6).

Separately, `commit_vote` always emits `approve(oracle, bondTarget)` regardless of the current allowance (`service.rs:226-232`), and F-SEN-006 shows the same `approve` is re-emitted on replay. For a token in the USDT family — one that requires the allowance to be reset to zero before a non-zero-to-non-zero change — a residual allowance left by a reverted `commit` makes the _next_ `approve` revert, which bricks every subsequent commit until an operator intervenes.

Both are conditional on the deployed fee token. Only a plain `MyToken` is exercised anywhere in the repository (`scripts/run_sentinel_integration_test.sh`), so neither path has ever been observed.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The approve gas limit is hard coded at 55,000 with no estimation | E2 | crates/sentinel/src/service.rs:676-688 | `    fn encode_action_kind(&self, kind: SentinelActionKind) -> Transaction {`<br>`        match kind {`<br>`            SentinelActionKind::ApproveToken { bond } => Transaction {`<br>`                to: self.fee_token,`<br>`                value: U256::ZERO,`<br>`                data: ERC20::approveCall {`<br>`                    spender: self.oracle,`<br>`                    amount: bond,`<br>`                }`<br>`                .abi_encode`<br>`                .into,`<br>`                gas: 55_000,`<br>`            },` |
| 2 | The oracle-call limit is 250,000, justified by a measurement against the reference contracts only | E2 | crates/sentinel/src/service.rs:689-704 | `            // Measured onchain at ~196k gas for a request's first commit (fresh`<br>`            // storage slots for the request, the commitment and the ERC-20`<br>`            // allowance spend); 100k undershot this and ran out of gas. 250k`<br>`            // keeps headroom for \`reveal\`/\`finalize\`/\`claim\`'s own cold-storage`<br>` // writes and the fee-token transfer.`<br>` SentinelActionKind::Commit { id, hash } => Transaction {`<br>` to: self.oracle,`<br>` value: U256::ZERO,`<br>` data: SentinelOracle::commitCall {`<br>` requestId: id,`<br>` commitHash: hash,`<br>` }`<br>` .abi_encode`<br>` .into,`<br>` gas: 250_000,`<br>` },` |
| 3 | The approve is unconditional — no allowance read, even though the binding exists and is unused | E2 | crates/sentinel/src/service.rs:226-233 | `        let actions = vec![`<br>`            SentinelAction {`<br>`                kind: SentinelActionKind::ApproveToken {`<br>`                    bond: U256::from(bond_target),`<br>`                },`<br>`                expires_at: Some(commit_deadline),`<br>`            }`<br>`            .into,` |
| 4 | `ERC20.allowance` is bound but never called | E2 | crates/sentinel/src/bindings.rs:60-64 | `        #[derive(Debug)]`<br>`        contract ERC20 {`<br>`            function approve(address spender, uint256 amount) external returns (bool);`<br>`            function allowance(address owner, address spender) external view returns (uint256);`<br>`        }` |
| 5 | The queue broadcasts the encoded gas limit verbatim; no estimation happens anywhere | E2 | crates/core/src/tx/mod.rs:246-248 | `        let chain_id = self.provider.chain_id;`<br>`        let fees = self.fees.await?;`<br>`        let transaction = transaction.build(chain_id, fees);` |
| 6 | The oracle pulls the bond through the fee token on commit, so the allowance must be exactly right | E2 (Solidity reference, A7) | contracts/src/SentinelOracle.sol:234-236 | `        uint96 bondAmount = request.applyCommit;`<br>`        $commitments.add(requestId, msg.sender, commitHash, bondAmount);`<br>`        FEE_TOKEN.safeTransferFrom(msg.sender, address(this), bondAmount);` |

## Trigger

- **Gas variant:** the deployment's `FEE_TOKEN` is a proxy (an extra `DELEGATECALL` plus proxy-slot reads) or writes more than the single allowance slot. Every `ApproveToken` transaction runs out of gas at 55,000; the following `commit` reverts inside `safeTransferFrom` for insufficient allowance (basis 6). The sentinel never commits to anything and burns 55,000 gas per proposal.
- **Allowance-reset variant:** the fee token requires `approve(spender, 0)` before any non-zero-to-non-zero change. Sequence: request A → `approve(bondTarget_A)` succeeds → `commit(A)` reverts for an unrelated reason (a duplicate from F-SEN-006, a `SentinelNotActive` from F-SEN-007, or simply losing the commit-window race) → allowance stays at `bondTarget_A` → request B → `approve(bondTarget_B)` **reverts** because the allowance is non-zero → `commit(B)` reverts → and so on for every subsequent request. Recovery requires an out-of-band `approve(oracle, 0)`.

Neither trigger is attacker-controlled; both are deployment-configuration dependent, which is why this is filed Low rather than Medium.

## Considered and rejected

- **"The 250,000 figure covers everything."** It is documented as measured for the _reference_ contracts and explicitly sized for `reveal`/`finalize`/`claim`'s cold-storage writes (basis 2); the concern here is the `approve` leg on a non-reference token, which the 250,000 budget does not apply to.
- **"The allowance can accumulate to an unsafe amount."** It cannot: `approve` assigns rather than adds, and the spender is always `self.oracle` (basis 1), so the outstanding allowance is at most one `bondTarget` and is spendable only by the oracle's `commit`, which requires `msg.sender == sentinel` (basis 6). This half of the concern is genuinely not a finding.
- **"A gas-estimating queue would fix it."** It would, but `core::tx` has no estimation path at all (basis 5), so this is a change of shape rather than a knob; noted as remediation option 2.
- **"The integration test proves the limits are right."** It proves them for `MyToken` deployed by `scripts/run_sentinel_integration_test.sh` only. There is no unit test covering `SentinelEncoder` output at all — no test in `crates/sentinel/src/service.rs` asserts either the calldata or the gas of an encoded action.
- **False positive check — is the fee token really operator-configured rather than read from the oracle?** Yes: `SentinelConfig::fee_token` (`config.rs:52-53`) is a required TOML field and is passed straight to `SentinelEncoder` (`service.rs:830`); the oracle's own `FEE_TOKEN` is never read (see F-SEN-007).

## Remediation options

1. **Estimate, or make the limits configurable.** Add per-action gas overrides to `[transactions]` in the config, or call `eth_estimateGas` once per action kind at startup (the token and oracle are fixed, so a single estimate per kind, cached, is enough) and apply a safety multiplier. Tradeoff: an estimate taken at startup can drift; a per-transaction estimate costs an RPC call per submission.
2. **Skip the approve when the allowance already suffices.** Read `ERC20.allowance(self, oracle)` (basis 4 — the binding already exists) via an effect and emit `ApproveToken` only when it is below `bondTarget`. Removes the redundant approve on replay (F-SEN-006) and halves the gas burn in the F-SEN-007 failure modes.
3. **Reset-then-set for non-standard tokens.** If the deployment's fee token is known to require it, emit `approve(oracle, 0)` before `approve(oracle, bondTarget)`. Tradeoff: an extra transaction per request on every deployment unless it is made conditional.
4. **Document the fee-token requirements.** State explicitly in the handbook and in `SentinelConfig`'s doc comment that the fee token must be a plain ERC-20 whose `approve` fits in 55,000 gas and permits non-zero-to-non-zero updates.

Tests to add: a `SentinelEncoder` unit test asserting the calldata and gas of each action kind (there is none today). An integration-script variant deploying a proxied or USDT-style token and asserting the sentinel still participates.

## Trail

- Reviewer R7: drafted from lead SEN-H8, self-estimate 50%. All six basis citations re-opened in this checkout. The code facts are E2; whether the deployed Gnosis fee token actually exceeds 55,000 gas or restricts `approve` is unknown from this checkout, so the impact is inference and the severity is Low. The prior analysis' "unlimited approval" concern is explicitly refuted above.

## Critic (C-SEN)

### Per-claim verdicts

All six basis rows re-opened; every quote is accurate (`service.rs:226-233`, `:676-688`, `:689-704`, `bindings.rs:60-64`, `core/tx/mod.rs:246-248`, `SentinelOracle.sol:234-236`). No claim marked `H`. The two structural facts are beyond dispute: the gas limits are literals with no estimation path anywhere, and `commit_vote` emits `approve(oracle, bond_target)` unconditionally with no `allowance` read — the `ERC20::allowance` binding at `bindings.rs:63` exists but is never called.

### Severity checked both ways — Low is right, and 50% is well calibrated

I looked specifically for an under-rating and did not find one:

- **Ordering does not break the assignment semantics.** With `N` concurrent requests each emitting `approve(bond)` then `commit`, the queue allocates sequential nonces and submits FIFO (`core/tx/mod.rs:204-216`, `core/tx/storage.rs:144-168`), so the pairs execute in order onchain and each `commit` consumes exactly the allowance the preceding `approve` set. An _assigning_ `approve` is therefore correct here, not a latent under-approval. This is the failure mode I expected to find and it is not present.
- **Both stated variants are conditional on a token nobody has named.** The 55,000-gas ceiling fits a plain assigning `approve` (~46k) with headroom for a simple proxy; it is only tight for a hooked or heavily-instrumented token. The USDT-family reset requirement needs a residual allowance, which needs a reverted `commit` first (F-SEN-007) or a replayed `approve` (F-SEN-006). Both are chains of conditionals on an unknown deployment.

So the mechanism is `E2` (the literals and the unconditional `approve` are facts) but the trigger is unproven and unprovable from this checkout — the fee token's identity is not in the repository, and only a plain `MyToken` appears in `scripts/`.

### Finding verdict

**Plausible. Certainty 52%. Severity Low (unchanged).**

Squarely in the 40-69 band: verified mechanism, unproven trigger. R7's own 50% self-estimate was honest and needed almost no adjustment — noted because it is the opposite of the usual defect. Low per Section 8 (a robustness/configuration weakness with limited impact); it would become Medium only if the deployed fee token is shown to be proxied, hooked, or reset-requiring, in which case the sentinel cannot commit at all.

### Notes for QA

This is settled by one fact, not a test: ask the team for the deployed `FEE_TOKEN` address and read its `approve` implementation. If it is a plain assigning OpenZeppelin ERC-20 behind no proxy, both variants close and this drops to Informational.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 2, then option 1. Option 3 should be conditional, not default.**

Option 2 (read `ERC20.allowance(self, oracle)` and emit `ApproveToken` only when it is below `bondTarget`) is sound and is the best value here: the `ERC20` binding already exists (`crates/sentinel/src/bindings.rs`), it removes the redundant `approve` on every replay (**F-SEN-006**), and it halves the gas burn in **F-SEN-007**'s failure modes. Same shape condition as every other chain read in this crate: it must be a `Command::Effect` + `Resume`, and a failed read must fall back to emitting the approve rather than skipping the commit.

Option 1 (per-action gas overrides in config, or one `eth_estimateGas` per action kind at startup with a safety multiplier) is sound. Of the two forms, the startup estimate is better than config overrides — the token and oracle are fixed for the process lifetime, so one estimate per kind is enough, and it removes a knob an operator would have to tune blind. Its drift risk is real but bounded; note the existing comment at `service.rs:668-673` records that 100k undershot and 250k was chosen with headroom, so the current values are measured rather than guessed and the urgency is low.

Option 3 (reset-then-set `approve(0)` before `approve(bondTarget)`) should be **conditional on the deployment's fee token**, not default: it adds a transaction per request on every deployment, and combined with **F-SEN-004**'s throughput ceiling that is a real cost for a hazard that may not apply. Gate it on a config flag, or on option 2's allowance read showing a non-zero existing allowance.

Option 4 (document the fee-token requirements — plain ERC-20, `approve` within 55,000 gas, non-zero-to-non-zero permitted) is the honest minimum and is currently absent from both the handbook and `SentinelConfig`'s doc comment.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — the encoder is byte-identical, only relocated

`SentinelEncoder::encode_action_kind` moved from `service.rs:676-740` to **`crates/sentinel/src/service.rs:820-886`** (the three new oracle-event handlers were inserted above it). Its contents are unchanged:

- `ApproveToken` → `gas: 55_000` (`service.rs:823-833`);
- `Commit` → `gas: 250_000` (`service.rs:839-848`), `Reveal` → `250_000` (`:849-867`), `Finalize` → `250_000` (`:868-875`, cited before as `:723-730`), `Claim` → `250_000` (`:876-883`).

No estimation and no configuration knob was added. The unconditional non-zero `approve` in `commit_vote` is likewise unchanged at `service.rs:226-232`, and F-SEN-006's replay duplication of that same `approve` still applies. Only a plain `MyToken` is still exercised anywhere in the repository.

**Certainty 52% and severity Low / Low unchanged.** Status left at `Critiqued`.
