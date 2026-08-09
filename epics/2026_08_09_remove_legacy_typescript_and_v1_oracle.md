# Plan: Remove Legacy TypeScript Implementation and V1 Sentinel Oracle

Component: `validator/` (TypeScript workspace, deleted), `contracts/` (Solidity), `crates/validator` (Rust), `explorer/` (React/TypeScript), `scripts/`, `.github/workflows/`, `docs/`.

---

## Overview

[Validator Oxidation (Rust Port)](https://app.notion.com/p/Validator-Oxidation-Rust-Port-35aa8a34f3b88059a954c4e086f229e1?pvs=21) and [Implement MVP Sentinel Reference Checks](https://app.notion.com/p/Implement-MVP-Sentinel-Reference-Checks-375a8a34f3b8803fb067c3e566385104?pvs=21) shipped `crates/validator` and `crates/sentinel` as full Rust replacements for the original TypeScript validator/sentinel (`validator/`, ~29,200 LOC across 197 files) and for the original single-phase `SentinelOracle` contract (replaced by the commit/reveal `SentinelOracleV2`). Both legacy stacks are now incompatible with the current onchain state and exist only as dead weight: `scripts/run_integration_test.sh`'s own comment already notes *"The TS sentinel can no longer complete a commit-reveal vote against SentinelOracleV2"* (`scripts/run_integration_test.sh:19-21`), and the contracts, scripts, and Rust `validator` crate already carry `TODO(A4)` breadcrumbs marking the v1→v2 rename this epic performs (e.g. `contracts/src/SentinelOracleV2.sol:12-13`, `contracts/script/DeploySentinelOracleV2.s.sol:9`, `contracts/test/SentinelOracleV2.t.sol:11`).

A second, independent cleanup rides along: the original consensus design let a validator group attest a Safe transaction directly (`Consensus.proposeTransaction`/`attestTransaction`, referred to in code as a `Transaction` **packet**) with no Sentinel review at all. Since MVP Sentinel Reference Checks, every transaction is meant to go through the oracle-backed path (`proposeOracleTransaction`/`attestOracleTransaction`, an `OracleTransaction` packet) instead, and the plain path has no test coverage anywhere in the repo — confirmed by grep, no test exercises `proposeTransaction`/`attestTransaction`/`TransactionProposed`/`TransactionAttested`. This epic removes that path too, everywhere it appears: Solidity, the Rust validator crate, and the explorer frontend (where it is a real, currently-reachable UI feature — proposing/tracking a Safe transaction with no oracle — not just dead code, so it gets its own review-focused phase).

Four independent legacy surfaces are removed, one is renamed, and one CI gap is closed:

1. The TypeScript `validator/` workspace and the legacy `scripts/run_integration_test.sh` it powers (which is also the **only** CI job that still runs the TS validator by default — `.github/workflows/integration.yml:10-33`).
2. `SentinelOracle`/`SentinelOracleCommitments`/`SentinelOracleRequests` (v1, single-phase) — deleted. `SentinelOracleV2`/`SentinelOracleCommitmentsV2`/`SentinelOracleRequestsV2` — renamed to the canonical (non-suffixed) names, since v2 is the only version anything still uses.
3. The non-oracle `Transaction` packet path — `Consensus.proposeTransaction`/`proposeBasicTransaction`/`attestTransaction`/`getTransactionAttestation*`/`getRecentTransactionAttestation*` and the matching Rust/explorer code — removed everywhere.
4. **CI gap**: today, `scripts/run_validator_port_integration_test.sh` (the only script that actually exercises the validator's genesis-keygen → attest → epoch-rollover happy path) is *not* wired into any CI job at all (confirmed: `test:integration:validator` appears only in `package.json`, `README.md`, `AGENTS.md`, never in a workflow file), and it depends directly on the TS workspace being deleted in step 1 (`npm run --workspace validator dev`). This epic rewrites it to run two Rust validator instances instead of one TS + one Rust, and adds a `validator-integration` CI job for it — landing the epic's "at least one bash-based happy-path validator integration test in CI" deliverable. `scripts/run_sentinel_integration_test.sh` already is such a test for the sentinel side and needs no changes (it has zero TS dependency and already targets `SentinelOracleV2`).

Documentation (`docs/configuration.md`, `docs/validator-handbook.md`) currently describes the TS validator's environment-variable configuration model (`validator/.env.sample`) even though the currently-shipped `ghcr.io/safe-research/safenet-validator` image is the Rust binary, which is TOML-file-configured (`crates/validator/src/config.rs`, `--config-file`). This epic rewrites those docs to match what's actually shipped, as its final phase.

---

## Architecture Decision

**Delete, don't deprecate.** Every surface in scope (TS workspace, v1 oracle contracts, non-oracle packet path) has a fully working replacement already merged and in active use. There is no migration window to preserve and no reason to keep a compatibility shim — per repo convention (`AGENTS.md`), unused code is cleaned up outright rather than flagged or feature-gated.

**Rename v2 → canonical in place, don't introduce a new canonical file.** `SentinelOracleV2.sol` becomes `SentinelOracle.sol` (same for its two libraries and matching script/test), rather than, say, leaving `V2` in place and only deleting v1. The repo already carries `TODO(A4)` comments at exactly the three v2 file sites stating this is the intended follow-up once v1 is gone, so this isn't a new design decision — it's executing a plan already recorded in the code.

**Split "remove non-oracle packet" into one phase per surface (Solidity, Rust validator crate, explorer), not one giant PR.** The three surfaces are independently compilable/testable (the Rust crate's `sol!` bindings are hand-written mirrors of the ABI, not generated from Solidity build output, and the explorer's ABI fragments are likewise hand-written — see Tech Specs), so there is no compile-time coupling forcing them into one PR. Removing them separately also isolates the one part of this cleanup that is a genuine user-facing feature removal (explorer: users can currently submit and track a plain, non-oracle transaction proposal) from the two parts that are pure dead-code removal (Solidity, Rust), so a reviewer evaluating "are we OK dropping this capability" isn't stuck also re-reviewing mechanical Solidity/Rust deletions.

**Fix the validator CI gap with a rewritten script targeting the already-deployed, already-unused `AlwaysApproveOracle`, not by merging it into the sentinel happy-path test.** `contracts/script/Deploy.s.sol:12-45` already deploys and logs an `AlwaysApproveOracle` (line 41, "AlwaysApproveOracle:" console.log at line 45) that nothing currently uses — `scripts/run_validator_port_integration_test.sh` calls `cmd:propose` (`Consensus.proposeTransaction`) instead. Since the validator happy path being tested here is genesis DKG → epoch rollover → transaction attestation, not the Sentinel dispute/commit-reveal machinery, swapping the propose call to `cmd:propose:oracle` (`proposeOracleTransaction`) against `AlwaysApproveOracle` — which synchronously approves inside the same transaction, so the validator observes `OracleTransactionProposed` and the oracle's approval together — gives a real oracle-backed happy path without dragging in Sentinel's commit/reveal state machine, and finally makes use of a value the deploy script has computed and logged since before this epic. This also decouples the validator-integration fix from the timing of the v1/v2 rename and the non-oracle-packet removal: `proposeOracleTransaction`/`AlwaysApproveOracle` already exist today, unrelated to either.

### Alternatives Considered

- **Keep `SentinelOracleV2` named as-is and just delete v1.** Rejected: leaves "V2" as the permanent name of the only version that exists, which is exactly the confusing state the repo's own `TODO(A4)` comments flag for cleanup. Renaming costs one mostly-mechanical PR (Tech Specs Phase 2) and removes a permanent footgun for every future reader who has to remember "there is no v1, V2 is current."
- **Merge the non-oracle-packet removal (Solidity + Rust + explorer) into a single PR since it's conceptually one change.** Rejected: none of the three are compile-coupled to each other (see Architecture Decision), the explorer piece is materially riskier (real UI feature loss) than the other two, and the combined diff would be large and heterogeneous (Solidity, Rust, TypeScript/React, tests in three languages) — harder to review than three same-language, single-purpose PRs, even though the repo's phase-size waiver ("ok if phases make more than 300 line changes if primarily removing code") would technically permit merging them.
- **Consolidate the sentinel and validator bash integration tests into one script/CI job.** Rejected: `scripts/run_sentinel_integration_test.sh` already passes today, is CI-wired, and tests a different subsystem (oracle dispute/commit-reveal) than the validator happy path (DKG/epoch rollover/attestation). Keeping them as two scripts/two CI jobs (mirroring each other's structure) keeps failures attributable to the right subsystem, the same way `sentinel-integration` and (the new) `validator-integration` would each fail independently rather than one undifferentiated job.
- **Leave `scripts/run_validator_port_integration_test.sh`'s name as-is.** Rejected: the filename's `_port_` specifically signals "compares the TS port against the Rust port," which stops being true once the TS side is replaced with a second Rust instance; keeping the name would mislabel what the script does going forward. Renamed to `scripts/run_validator_integration_test.sh` (Tech Specs Phase 2), matching the existing `run_sentinel_integration_test.sh` naming pattern.
- **Rewrite `docs/configuration.md`/`docs/validator-handbook.md` earlier, alongside Phase 1 (TS deletion).** Rejected: those docs should describe the *final* state of the Rust validator's configuration surface, and Phases 2-6 don't touch `crates/validator/src/config.rs`'s schema, but sequencing the docs rewrite after all code phases avoids re-touching the same doc twice if anything in those phases turns out to need a doc callout (e.g. the `validator-integration` CI job, the renamed contract, the removed non-oracle getters that a few doc code samples reference).

---

## Tech Specs

### Phase 1 — Remove the TypeScript validator/sentinel workspace and its legacy integration test

- Delete the entire `validator/` directory (197 files, ~29,200 LOC: `src/`, `bin/`, `migrations/`, `Dockerfile`, `Dockerfile.dockerignore`, `README.md`, `biome.json`, `package.json`, `tsconfig*.json`, `vitest.config.ts`, `.env.sample`). Confirmed nothing outside `validator/` imports `@safenet/validator` (only `package-lock.json`'s resolved-dependency block references it), and `validator/Dockerfile` is not built by `.github/workflows/docker.yml` (its matrix only covers `crates/validator/Dockerfile`, `crates/sentinel/Dockerfile`, `contracts/Dockerfile`) — safe to delete outright.
- Delete `scripts/run_integration_test.sh`.
- `.github/workflows/integration.yml`: delete the `integration` job (lines 10-33), which runs `npm run test:integration`. Keep the `sentinel-integration` job (lines 35-67) unchanged — it has no TS dependency.
- Root `package.json`: remove `"validator"` from the `workspaces` array (`package.json:8-12` becomes 3 entries); remove the `test:integration` script entry (`package.json:18`, or wherever it lands after the workspaces edit). Keep `test:integration:sentinel` and `test:integration:validator` (the latter is fixed up in Phase 2).
- Run `npm install` (not `npm ci`, since `package-lock.json` must be regenerated to drop the `@safenet/validator` workspace entries) and commit the updated lockfile.
- Root `README.md`: remove the `- [Validator](./validator) Validator service (Typescript & npm)` bullet; remove/rewrite the "Integration test" section's `npm run test:integration` example to point only at the two remaining commands.
- `AGENTS.md`: remove the `validator/` architecture bullet describing it as "Node.js + TypeScript validator..."; rewrite the "Integration Tests" table to drop the `test:integration` row and correct the description of `test:integration:sentinel`/`test:integration:validator` (the current text incorrectly claims `test:integration:sentinel` also exercises the TS validator — it doesn't, per direct inspection of the script).
- Leave `docs/overview.md:205`'s footnote link alone — it's a permalink pinned to a historic commit SHA in the TS source tree, harmless once `validator/` is gone from the current tree, and not worth touching (see Assumptions).
- **This phase is dominated by deletion and is exempt from the 300-line/10-file PR guideline per the epic's own scope note ("ok if phases make more than 300 code line changes if this is primarily removing code").**

### Phase 2 — Fix the validator CI gap: Rust-only happy-path integration test, wired into CI

Depends on Phase 1 (removes the TS process this script currently spawns). Otherwise independent of every other phase — `proposeOracleTransaction`/`AlwaysApproveOracle` already exist today and are untouched by Phases 3-6.

- Rename `scripts/run_validator_port_integration_test.sh` → `scripts/run_validator_integration_test.sh` (drop `_port_`, matching `run_sentinel_integration_test.sh`'s naming).
- Replace the TypeScript validator process (`npm run --prefix "$REPO_ROOT" --workspace validator dev`, old line 108) with a second Rust validator instance, built once (`cargo build --package validator`, already done at old line 65) and run twice with distinct TOML configs/participant indices — mirroring `run_sentinel_integration_test.sh`'s two-`cargo run`-instances pattern (`scripts/run_sentinel_integration_test.sh:154,158`) and `run_devnet.sh`'s per-instance TOML generation (`scripts/run_devnet.sh:319-348`) as the template for building each config.
- Replace both `cmd:propose` invocations (old lines 197, 254 — `Consensus.proposeTransaction` via `ProposeScript`) with `cmd:propose:oracle` (`Consensus.proposeOracleTransaction` via `ProposeOracleTransactionScript`, `contracts/script/ProposeOracleTransaction.s.sol:10`), passing `ORACLE_ADDRESS` set to the `AlwaysApproveOracle` address that `DeployScript` already deploys and logs (`contracts/script/Deploy.s.sol:41,45`) — parse it from the deploy step's broadcast output the same way the script already parses `CONSENSUS_ADDRESS`.
- Update log-grepping/assertions (old lines 208, 219, 265, 286) from `TransactionProposed`/`TransactionAttested` to `OracleTransactionProposed`/`OracleTransactionAttested`; the success condition (old lines 292-295: both a genesis-epoch and an epoch-1 transaction attested, plus `KeyGenConfirmed`/`EpochStaged`/`EpochRolledOver`) stays conceptually the same, just against the oracle-backed events.
- Root `package.json:23`: no change needed to the script name (`test:integration:validator`), only to what file it points at.
- `.github/workflows/integration.yml`: add a `validator-integration` job modeled on the existing `sentinel-integration` job (lines 35-67) — checkout, setup-node, foundry-toolchain pinned to v1.5.1, `npm ci`, `npm run test:integration:validator`, upload relevant log artifacts on failure (mirror whatever log files the rewritten script writes, analogous to `anvil_sentinel_logs.txt`/`sentinel_a_logs.txt`/`sentinel_b_logs.txt`).
- `README.md`/`AGENTS.md`: update the `test:integration:validator` description (no longer "Rust validator, alongside the TypeScript one" — now "two Rust validator instances, against an `AlwaysApproveOracle`-backed happy path, running in CI").
- **Acceptance bar for this phase**: `validator-integration` passes green in CI on the PR itself (workflow triggers on `pull_request`, `.github/workflows/integration.yml:2-3`), and `sentinel-integration` remains unaffected. This closes the epic's CI deliverable.

### Phase 3 — Remove v1 `SentinelOracle`; rename v2 to canonical

Independent of Phases 1-2 and 4-5; can run in parallel with any of them.

- Delete: `contracts/src/SentinelOracle.sol`, `contracts/src/libraries/SentinelOracleCommitments.sol`, `contracts/src/libraries/SentinelOracleRequests.sol`, `contracts/script/DeploySentinelOracle.s.sol`, `contracts/test/SentinelOracle.t.sol`. Confirmed exhaustively: none of these five files are referenced anywhere outside each other and this list.
- Rename (file + primary contract/library identifier):
  - `contracts/src/SentinelOracleV2.sol` → `contracts/src/SentinelOracle.sol`; `contract SentinelOracleV2` → `SentinelOracle` (drop the `TODO(A4)` comment at lines 12-13, it's now done).
  - `contracts/src/libraries/SentinelOracleCommitmentsV2.sol` → `contracts/src/libraries/SentinelOracleCommitments.sol`.
  - `contracts/src/libraries/SentinelOracleRequestsV2.sol` → `contracts/src/libraries/SentinelOracleRequests.sol`.
  - `contracts/script/DeploySentinelOracleV2.s.sol` → `contracts/script/DeploySentinelOracle.s.sol`; `contract DeploySentinelOracleV2Script` → `DeploySentinelOracleScript` (drop its `TODO(A4)` at line 9).
  - `contracts/test/SentinelOracleV2.t.sol` → `contracts/test/SentinelOracle.t.sol`; `contract SentinelOracleV2Test` → `SentinelOracleTest` (drop its `TODO(A4)` at line 11).
- Fix every inbound reference to the renamed identifiers/paths:
  - `contracts/package.json:12` — `cmd:deploy:sentinel-oracle` script string (`DeploySentinelOracleV2Script` → `DeploySentinelOracleScript`).
  - `scripts/run_devnet.sh:301,308,392` — script-name references.
  - `scripts/run_sentinel_integration_test.sh:84` — broadcast artifact path (`build/broadcast/DeploySentinelOracleV2.s.sol/...` → `.../DeploySentinelOracle.s.sol/...`).
  - `docs/devnet.md:317` (and prose mentions at lines 42, 93, 125, 152, 218, 234, 303) — broadcast path and prose.
  - `crates/sentinel/src/bindings.rs:8` and `crates/sentinel/src/hashing.rs:17,159,162` — doc-comment cross-references to the old `*V2.sol`/`SentinelOracleV2.t.sol` paths (no code change needed in `crates/sentinel` beyond these comments — its `sol!` bindings and identifiers were already canonical, matching only v2's ABI, per direct inspection).
  - `explorer/src/lib/oracle/abi.ts` — rename `sentinelOracleV2Abi` → `sentinelOracleAbi` (plus its "V1 is deprecated" comment, now stale/removable), propagate the rename through `explorer/src/lib/oracle/votes.ts`, `explorer/src/lib/oracle/votingStatus.ts`, `explorer/src/lib/oracle/votes.test.ts`, and update the `SentinelOracleV2`-mentioning comments in `explorer/src/hooks/useSentinelVotes.tsx`.
- No `contracts/test/Consensus.t.sol` or other test-file changes needed beyond the rename above — nothing else references either oracle version.
- `AGENTS.md:79` prose mention of `SentinelOracleV2`: reword to drop "V2".

### Phase 4 — Remove the non-oracle `Transaction` packet path: Solidity

Independent of Phases 1-3 and 5; can run in parallel with any of them.

- `contracts/src/Consensus.sol`: delete `proposeTransaction` (307-314), `proposeBasicTransaction` (319-342), `attestTransaction` (365-384), `getTransactionAttestation` (252-258), `getTransactionAttestationByHash` (263-270), `getRecentTransactionAttestation` (275-281), `getRecentTransactionAttestationByHash` (286-302), and the `attestTransaction`-selector branch of the `onSignCompleted` dispatch (451-454) — leave the surrounding if/else chain and every oracle/epoch function untouched.
- `contracts/src/interfaces/IConsensus.sol`: delete the matching declarations — `event TransactionProposed` (75-81), `event TransactionAttested` (92-99), `getTransactionAttestation` (216-219), `getTransactionAttestationByHash` (227-230), `getRecentTransactionAttestation` (241-244), `getRecentTransactionAttestationByHash` (252-255), `proposeTransaction` (262), `proposeBasicTransaction` (275-282), `attestTransaction` (293-299).
- `contracts/src/libraries/ConsensusMessages.sol`: delete `TRANSACTION_PROPOSAL_TYPEHASH` (26-30) and `transactionProposal` (88-110) — both used only by the functions above.
- Delete `contracts/script/Propose.s.sol` (exists solely to call `proposeTransaction`) and its `contracts/package.json:10` `cmd:propose` entry (keep `cmd:propose:oracle`, used by Phase 2's rewritten script).
- No test deletions needed in `contracts/test/Consensus.t.sol` or `contracts/test/libraries/ConsensusMessages.t.sol` — confirmed neither file has a test for any function/typehash being removed here; only verify both files still compile against the shrunk `Consensus`/`IConsensus`/`ConsensusMessages` surface.
- `docs/devnet.md`: remove the "Proposing a Safe transaction" plain-propose walkthrough (lines 134-150) and the `getRecentTransactionAttestationByHash`/`getTransactionAttestationByHash` cast examples (lines 201, 210), keeping the oracle-flow equivalents already present (lines 152-177, 213).
- `docs/overview.md:185`: update the attestation state-diagram edge label from `EpochStaged()<br>TransactionAttested()` to `EpochStaged()<br>OracleTransactionAttested()`.
- `examples/`: `examples/README.md:10-11` reword away from "via `proposeBasicTransaction`"; `examples/attest-safe-tx.ts` — its `pollAttestation` step (168-190) currently polls `getRecentTransactionAttestationByHash` (176) and declares that ABI item (119-121); update it to poll the oracle-variant getter (`getOracleTransactionAttestationByHash`) instead, consistent with the rest of the epic.

### Phase 5 — Remove the non-oracle `Transaction` packet path: Rust validator crate

Independent of Phases 1-4 and 6; can run in parallel with any of them (the `sol!` bindings here are hand-written, not generated from Solidity build output, so this has no compile dependency on Phase 4).

- `crates/validator/src/bindings.rs`: delete `event TransactionProposed` (133-139), `event TransactionAttested` (140-147), `function attestTransaction` (174-180) from the `sol!` `Consensus` block. Update the doc comment at line 44 ("as carried by the `(Oracle)TransactionProposed` events") to drop the `(Oracle)` alternation, since only `OracleTransactionProposed` remains.
- `crates/validator/src/consensus/hashing.rs`: delete `struct TransactionProposal` (36-39), `transaction_proposal_hash` (111-117), `transaction_packet_hash` (119-123), and the `reference_transaction_packet_hash` test (207-213). Keep everything oracle/epoch/shared (`OracleTransactionProposal`, `oracle_transaction_proposal_hash`, `oracle_transaction_packet_hash`, `SafeTx`/`safe_tx_hash`, `EpochRollover`/`epoch_rollover_hash`, `ConsensusDomain`) untouched.
- `crates/validator/src/state/mod.rs`: delete the `Transaction { epoch, transaction }` variant of `enum Packet` (234-240); delete the two dispatch arms `Event::Consensus(Consensus::ConsensusEvents::TransactionProposed(..))` and `...TransactionAttested(..)` (435-440) from the transition match, leaving `EpochStaged`/`OracleTransactionProposed`/`OracleTransactionAttested`/`OracleResult`/catch-all arms untouched.
- `crates/validator/src/state/transactions.rs`: delete `handle_transaction_proposed` (13-71) and `handle_transaction_attested` (79-95) in full. Reword the doc comment on `handle_oracle_transaction_proposed` (99) that currently cross-references "a plain `[Consensus::TransactionProposed]`" — that type no longer exists.
- `crates/validator/src/service/action.rs`: delete the `Action::AttestTransaction { .. }` variant (88-98) and its encoder arm building `Consensus::attestTransactionCall` (345-368). Keep `AttestOracleTransaction`, `StageEpoch`, `SetValidatorStaker`, and every keygen/preprocess/sign action untouched.
- `crates/validator/src/state/sign.rs` — four sites, none of them a blanket deletion:
  - `handle_sign` (94): narrow the combined arm `Packet::Transaction { .. } | Packet::EpochRollover { .. } =>` to `Packet::EpochRollover { .. } =>` only; the branch body is unchanged (still needed for `EpochRollover`).
  - `Packet::epoch()` (801): narrow `Packet::Transaction { epoch, .. } | Packet::OracleTransaction { epoch, .. } => *epoch` to `Packet::OracleTransaction { epoch, .. } => *epoch` only.
  - `Packet::attestation_callback` (812): delete the `Packet::Transaction { epoch, transaction } => (*epoch, None, transaction),` arm; once only `OracleTransaction` remains, simplify the now-single-arm match and the downstream `match oracle { None => .., Some(oracle) => .. }` (839-857) — the `None` branch (building `Consensus::attestTransactionCall`) becomes dead and the `Option<Address>` plumbing through this call chain (810, 811, 839) can collapse to a plain `Address`.
  - `Packet::attestation_action` (881): delete the `Packet::Transaction { epoch, transaction } => Action::AttestTransaction { .. }` arm; `EpochRollover` and `OracleTransaction` arms are self-contained and unaffected.
  - Reword the doc comments at 367-370, 492, 806 that enumerate `attestTransaction` alongside `stageEpoch`/`attestOracleTransaction`.
- `crates/validator/src/state/keygen.rs`: no change — its only `Packet` reference is `Packet::EpochRollover`, unrelated.

### Phase 6 — Remove the non-oracle `Transaction` packet path: explorer frontend

Depends on Phase 4 landing first (removes the UI for onchain functions that no longer exist), independent of Phases 1-3, 5.

This is a real, currently-reachable UI feature removal (a user can submit and track a plain, non-oracle transaction proposal today), not mechanical dead-code deletion, so it gets its own focused review pass:

- `explorer/src/lib/consensus/abi.ts`: remove the `proposeTransaction` and `getTransactionAttestationByHash` ABI entries and the `TransactionProposed`/`TransactionAttested` event entries (8-11); drop the two non-oracle names from `transactionEventSelectors` (18-23) and `proposedEventSelectors` (25-27).
- `explorer/src/lib/consensus/transactions.ts`: `loadProposedSafeTransaction` (58-91) and `loadTransactionProposals` (93-196) currently branch on both oracle and non-oracle event kinds at every step (lines 124, 145, 149, 159, 164, 173) — remove the non-oracle branch at each; the `TransactionProposal.oracle` field (type at 38-46) becomes non-nullable `Address` once every proposal is oracle-backed.
- Delete `explorer/src/lib/packets.ts` in full (implements `safeTxProposalHash`, the client-side mirror of the now-deleted `ConsensusMessages.transactionProposal`); its only caller, `explorer/src/lib/coordinator/signing.ts:116-130`, loses its `oracle == null ? .. : safeTxProposalHash(..)` branch and keeps only the oracle path.
- `explorer/src/components/transaction/SafeTxProposals.tsx:111`: remove the `proposal.oracle != null && <SafeTxProposalVoting .../>` null-check now that `oracle` is always present; review `useSubmitProposal.tsx`/`postTransactionProposal` (`transactions.ts:198-206`) for any relayer call that still assumes a non-oracle submission path.
- Update/delete the tests exercising the plain path specifically: `explorer/src/lib/consensus/consensus.test.ts:166-268` (drop the `"TransactionProposed"|"TransactionAttested"` cases and the dedicated `"returns the transaction from a plain TransactionProposed log"` test at 255), `explorer/src/lib/coordinator/signing.test.ts:83` (`"uses the plain TransactionProposal hash..."`), and the `oracle: null` fixtures in `explorer/src/routes/safe.test.tsx:87`, `explorer/src/components/transaction/TransactionProposalsList.test.tsx:26`, `explorer/src/components/transaction/SafeTxProposals.test.tsx:69`.
- Manually verify in a browser (per repo convention for frontend changes) that the explorer still loads a transaction list and an oracle-backed proposal's voting UI correctly after the non-nullable `oracle` field change.

### Phase 7 — Documentation pass: Rust validator configuration

Depends on Phases 1-6 (documents the final state).

- `docs/configuration.md`: replace the "Environment Variables" section (38-56, and its link to the now-deleted `../validator/.env.sample`) with the Rust validator's actual TOML schema — `crates/validator/src/config.rs`'s `Config`/`ValidatorConfig`/`Participant` structs (`rpc`, `signer`, `database`, `validator.{consensus, staker, participants, oracles, genesis_salt, blocks_per_epoch, key_gen_timeout, signing_timeout, oracle_timeout}`, `observability`, `driver`), using `scripts/run_devnet.sh:319-348`'s generated-TOML shape as a concrete worked example.
- `docs/validator-handbook.md`: replace the `cp validator/.env.sample validator/.env` / `docker run --env-file validator/.env ...` instructions (lines 82-92) with the actual `--config-file`-based invocation matching `crates/validator/Dockerfile`'s `ENTRYPOINT ["./validator"]` and its documented `args: [--config-file=...]` override (`crates/validator/Dockerfile:35`).
- Sweep `README.md`/`AGENTS.md` for any remaining stale mentions introduced or missed by Phases 1-6 (e.g. confirm the "Rust Port" section of `README.md` still accurately lists all three `npm run test:integration*` commands post-rename).
- Leave `docs/overview.md:205`'s SHA-pinned historic permalink alone (see Assumptions).

### Phase 8 — Remove this plan

Delete `epics/2026_08_09_remove_legacy_typescript_and_v1_oracle.md` once Phases 1-7 are merged.

---

## Implementation Phases

| Phase | Summary | Depends on | Own PR |
|---|---|---|---|
| 1 | Delete `validator/` TS workspace, `scripts/run_integration_test.sh`, the `integration` CI job; update `package.json`/lockfile/README/AGENTS.md | — | ✅ |
| 2 | Rewrite `run_validator_port_integration_test.sh` → `run_validator_integration_test.sh` (two Rust instances, oracle-backed propose via `AlwaysApproveOracle`); add `validator-integration` CI job | 1 | ✅ |
| 3 | Delete v1 `SentinelOracle`/libraries/script/test; rename v2 → canonical everywhere (contracts, scripts, docs, explorer identifiers, Rust doc comments) | — (parallel with 1, 2, 4, 5) | ✅ |
| 4 | Remove non-oracle `Transaction` packet path: Solidity (`Consensus.sol`, `IConsensus.sol`, `ConsensusMessages.sol`, `Propose.s.sol`, docs, examples) | — (parallel with 1, 2, 3, 5) | ✅ |
| 5 | Remove non-oracle `Transaction` packet path: Rust validator crate (`bindings.rs`, `hashing.rs`, `state/mod.rs`, `state/transactions.rs`, `service/action.rs`, `state/sign.rs`) | — (parallel with 1, 2, 3, 4) | ✅ |
| 6 | Remove non-oracle `Transaction` packet path: explorer frontend (real UI feature removal) | 4 | ✅ |
| 7 | Rewrite `docs/configuration.md`/`docs/validator-handbook.md` for the Rust validator's TOML config; final README/AGENTS.md sweep | 1, 2, 3, 4, 5, 6 | ✅ |
| 8 | Remove this plan | 7 | ✅ |

Phases 1, 3, 4, 5 have no dependency on each other and can be implemented and reviewed in parallel; Phase 2 only needs Phase 1; Phase 6 only needs Phase 4; Phase 7 is a documentation-only pass that should land last so it describes the epic's end state; Phase 8 closes it out.

---

## Open Questions and Assumptions

- **`docs/overview.md:205`'s historic permalink into the TS source tree is left untouched.** It's pinned to a specific past commit SHA, so it keeps resolving correctly on GitHub regardless of what the current tree contains; rewriting it to point at Rust source would change its meaning (it's cited as a specific historical implementation detail), not just its target.
- **The exact shape of the rewritten `scripts/run_validator_integration_test.sh`** (how the second Rust validator instance's TOML config is generated/parametrized, exact log file names for the new `validator-integration` CI job's failure-artifact upload) **is intentionally left loose**, to be settled during Phase 2's implementation/review rather than gated on this planning doc — consistent with how the referenced `2026_07_24_nonblocking_effects.md` epic scoped its own Rust-shape details.
- **`AlwaysApproveOracle` is assumed sufficient for the validator happy-path test** (Architecture Decision) rather than reusing any part of the Sentinel commit/reveal flow. If a future reviewer wants the validator integration test to also prove the validator behaves correctly under a *disputed*/denied oracle result, that is out of scope here and would be a follow-up, not a blocker for this epic's CI deliverable.
- **`contracts/test/Consensus.t.sol` and `contracts/test/libraries/ConsensusMessages.t.sol` need no test deletions** in Phase 4, per direct inspection — flagged here so Phase 4's reviewer doesn't go looking for tests to remove and wonder if something was missed.
- **No new "V2"-suffixed contract is introduced to replace the one being canonicalized.** If a future breaking change to the oracle contract is needed, it should follow whatever versioning convention the team adopts at that time — this epic only resolves the *existing* v1/v2 split, it doesn't prescribe how the next one should be named.
