# Phase 8 brief — real-world scenario validation

Repo root: `/home/shebin.guest/safe/safenet`. Commit `2893917`.

## Purpose

Phases 5–7 proved findings **mechanically**: unit tests with mock transports, hand-built fixtures, and three Anvil suites. This phase asks a harder question:

> **Would this actually happen to a running deployment?**

Real contracts on a local chain, real binaries, real timing, real gas, real multi-participant interaction. A defect that reproduces in a mock but cannot arise in a live system is worth **less** than its current certainty says, and saying so is a success of this phase, not a failure.

## HARD SAFETY BOUNDARY — read twice

**Local simulation only. Anvil on this machine. Never a testnet, never mainnet, never any live RPC.**

The repository will point you at live chains if you are careless. Known hazards, verified:

| Hazard | Detail |
| --- | --- |
| `crates/validator/validator.sample.toml:10` | `rpc = "https://rpc.gnosischain.com"` — **live Gnosis mainnet** |
| `crates/sentinel/sentinel.sample.toml:10` | same |
| `crates/sentinel-engine/sentinel-engine.sample.toml:12` | same |
| `scripts/run_sentinel_engine_integration_test.sh:11` | `RPC_URL` defaults to `https://ethereum-rpc.publicnode.com` — **live Ethereum mainnet** — unless `SENTINEL_ENGINE_RPC_URL` is set |

Rules that follow:

1. **Never use a sample config unmodified.** Copy it, rewrite `rpc` to the local anvil endpoint, and confirm the value before starting any binary.
2. **Before starting any service, print its effective `rpc` value** into your log so the evidence shows it was local. A run whose log does not show a `127.0.0.1`/`localhost` endpoint is not acceptable evidence.
3. **Never run `run_sentinel_engine_integration_test.sh`** unless `SENTINEL_ENGINE_RPC_URL` is explicitly set to a local anvil endpoint. It is blocked anyway (A8: no corpus), so prefer not running it at all.
4. **No `cast`/`forge` command against any `--rpc-url` that is not local.** No `--fork-url` against a live endpoint. No network access beyond the Cargo registry.
5. If a scenario genuinely cannot be built without live chain data, **do not build it** — record it as not testable under the safety boundary and say what would be needed.

## Method

- Deploy the real contracts with `forge script` against anvil (chain id 31337), as `scripts/run_validator_integration_test.sh` does. Drive the **real binaries**, not mocks.
- Reuse `scripts/lib` and the existing harnesses as a starting point, but **copy any script you need to modify into the session scratchpad** — never edit a tracked file.
- Where a finding needs a reorg, use `anvil_reorg`. Note what Phase 7 established: **`anvil_reorg` produces empty replacement blocks, so reorged transactions are dropped permanently and there is no log replay** — several findings' triggers depend on replay, and that limitation is itself a finding about testability, not evidence the defect is absent.
- Where a finding needs a restart, actually stop and restart the process. Phase 7 found the repo's own reorg-nonce harness **claims** a restart it never performs — do not inherit that mistake.

## What to record

Append a `## Real-world validation (Phase 8, <your agent name>)` section to each finding:

- The scenario: what was deployed, what was run, what was induced.
- **The verbatim observable outcome** — logs, chain state, exit codes.
- One of: **Reproduced end-to-end** / **Reproduced only under conditions unlikely in production** (say which conditions) / **Did not reproduce** / **Not testable locally** (say precisely why).
- The certainty and severity you set, and whether either moved.

**You may lower severity and certainty, and you should where the evidence warrants.** A finding whose trigger needs an operator to do something they would never do, or a chain state that cannot occur, is over-rated and this is where that gets fixed. Equally, a finding that reproduces end-to-end on the first attempt deserves to say so plainly.

Keep every finding-header table row terminated with a trailing `|`.

## Boundaries

Write only under `rust-audit/`. **Never modify a tracked file** — copy to the scratchpad and modify the copy. Never commit, branch, stash or push. Kill every process you start; leave nothing listening on 8545/8546 (a stray mock RPC from an earlier phase caused a false failure once already). At the end verify `git status --short` from the repo root shows nothing outside `rust-audit/`, and report it.
