# Plan: Migrate `run_devnet.sh` to the Rust Services

Component: `scripts/run_devnet.sh`, `crates/validator`, `crates/sentinel`.

---

## Overview

`scripts/run_devnet.sh` (`npm run devnet`) is the only devnet-style entry point in the
repo that still runs the TypeScript validator: it builds `localhost/safenet-validator`
from `validator/Dockerfile` (`run_devnet.sh:68`) and configures it entirely through
container environment variables (`RPC_URL`, `PRIVATE_KEY`, `CONSENSUS_ADDRESS`,
`PARTICIPANTS`, ..., `run_devnet.sh:116-140`). The Rust ports (`crates/validator`,
`crates/sentinel`) already exist and are already exercised by
`scripts/run_validator_port_integration_test.sh` and
`scripts/run_sentinel_integration_test.sh`, but neither has a `Dockerfile`, and neither
is configured through environment variables at all — both load a single TOML file via
`--config-file` (`crates/validator/src/main.rs`, `crates/sentinel/src/config.rs`), with
`deny_unknown_fields` structs (no env var escape hatch exists or is being added by this
epic).

This epic:

1. Adds the missing, production-usable `Dockerfile`s for `crates/validator` and
   `crates/sentinel` (neither exists today — only `validator/Dockerfile`, the
   TypeScript one, and `contracts/Dockerfile` exist).
2. Rewrites `run_devnet.sh` to build and run those Rust images instead of the
   TypeScript validator image, generating a TOML config per instance instead of an
   env var block.
3. Extends the devnet to also deploy a `SentinelOracleV2` and spin up two Rust sentinel
   containers voting on it — a new capability, not something `run_devnet.sh` does
   today in any form (only `run_sentinel_integration_test.sh` exercises the Rust
   sentinel, and only against a `TestConsensus` stand-in, not a real devnet
   `Consensus`).

This is scoped to `run_devnet.sh` and its supporting Dockerfiles/scripts only. It does
not remove any TypeScript code and does not change `scripts/run_integration_test.sh`
(TypeScript validator + sentinel) — that implementation, and the corresponding cleanup
of the oracle contracts down to a single version, is explicitly deferred to a separate,
later epic (see Assumptions). Wiring the new Dockerfiles into
`.github/workflows/docker.yml` for publishing to `ghcr.io` is likewise deferred to a
separate epic; this epic only needs them to be genuinely production-shaped so that
follow-up is a small, mechanical addition rather than a rewrite.

---

## Architecture Decision

**Config delivery: generate a TOML file per instance, mount it into the container, pass
`--config-file`.** Both Rust services already have exactly one supported configuration
path — a TOML file on disk — used identically by both existing Rust integration test
scripts (`run_sentinel_integration_test.sh`'s `sentinel_config()` heredoc,
`run_validator_port_integration_test.sh`'s `$RUST_CFG` heredoc). `run_devnet.sh` follows
the same pattern: for each validator/sentinel instance it writes a TOML file into a
per-run temporary directory (`mktemp -d`), and the generated pod spec mounts that
directory into the container via a `hostPath` volume, with the container's `args`
pointing `--config-file` at the mounted path.

- **Alternative rejected: teach the Rust config loaders to also read environment
  variables**, mirroring the TypeScript `zod`-validated env schema
  (`validator/src/types/schemas.ts`). Rejected because it would add a second,
  devnet-only configuration surface to production service code (both configs are
  `#[serde(deny_unknown_fields)]` specifically so a typo or stale field fails loudly) for
  a need that already has an established, working answer two other scripts already use.
- **Alternative rejected: embed the podman pod's dynamic values (contract addresses,
  private keys) via `podman kube play`'s `envFrom`/inline env instead of a mounted
  file.** `podman kube play` pod specs don't support the Rust services' actual
  configuration surface (a nested TOML document — participant lists, driver/index
  settings) as environment variables in the first place; a mounted file is the only
  option that doesn't also require inventing an env-var config path (the previous
  point).

**The new `Dockerfile`s are the production images, full stop — not a devnet-only
variant.** There is exactly one `Dockerfile` per crate, usable as-is for a future
`ghcr.io` publish (deferred to a separate epic, see Overview); the devnet is simply its
first consumer. This mirrors `validator/Dockerfile`'s existing shape: a builder stage
that compiles a release binary and a slim runtime stage that only copies the binary
across, with no devnet-specific shortcuts (debug tooling, relaxed permissions, etc.)
baked in.

**Image tags stay podman-local and separate from the published TypeScript image.** The
existing `localhost/safenet-validator` tag (`run_devnet.sh:68`) is built fresh by
`--build` on every devnet run and has no other consumer (`grep` confirms only
`run_devnet.sh` references it) — reusing it for the new Rust image is safe and keeps the
pod spec unchanged in shape. This is unrelated to and does not touch
`ghcr.io/safe-research/safenet-validator`, which `.github/workflows/docker.yml` builds
and publishes from `validator/Dockerfile` (the TypeScript one) on every push to `main`
and every tag — a real release artifact this epic has no reason to touch. A new
`localhost/safenet-sentinel` tag is introduced for the Rust sentinel image, which has no
prior tag to collide with.

**Rust `Dockerfile`s build from the repo root context, workspace-wide.** Both crates
depend on `safenet-core`/`safe-tx` via workspace path dependencies
(`Cargo.toml`'s `[workspace] members = ["crates/*"]`), so, like `contracts/Dockerfile`,
each new `Dockerfile` is built with the repo root as context (`podman build -f
crates/validator/Dockerfile .`) and needs a `Dockerfile.dockerignore` allow-listing only
`Cargo.toml`, `Cargo.lock`, and `crates/**` — mirroring `contracts/Dockerfile.dockerignore`'s
allow-list style, not `validator/Dockerfile.dockerignore`'s deny-list style. This matters
concretely: `target/` alone is ~700MB locally and must never enter the build context.
Neither crate's `bindings.rs` reads contract artifacts at build time (both use inline
`alloy::sol! {...}` macros, not an ABI file), so the contracts submodule and `contracts/`
build output are not needed in the Rust build context at all.

**Devnet identities are hardcoded, well-known Anvil test-mnemonic accounts, one per
role, none reused across roles.** `VALIDATORS` already hardcodes accounts `(1)`
(`alice`) and `(2)` (`bob`); this epic adds two sentinels using accounts `(3)`
(`carol`) and `(4)` (`dave`), and a `SentinelOracleV2` arbitrator using account `(5)` —
all from the standard Anvil test mnemonic, verified locally against a running `anvil`
rather than transcribed from memory:

| Role | Account | Address | Private Key |
|---|---|---|---|
| deployer/sender (existing) | `(0)` | `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` | `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80` |
| `alice` (existing) | `(1)` | `0x70997970C51812dc3A010C7d01b50e0d17dc79C8` | `0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d` |
| `bob` (existing) | `(2)` | `0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC` | `0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a` |
| `carol` (new, sentinel) | `(3)` | `0x90F79bf6EB2c4f870365E785982E1f101E93b906` | `0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6` |
| `dave` (new, sentinel) | `(4)` | `0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65` | `0x47e179ec197488593b187f80a00eb0da91f1b9d0b13f8733639f19c30a34926a` |
| arbitrator (new) | `(5)` | `0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc` | `0x8b3a350cf5c34c9194ca85829a2df0ec3153be0318b5e2d3348e872092edffba` |

Two sentinels is deliberately more than the bare minimum of one (which would trivially
"agree with itself" and never exercise a real commit-reveal vote across independent
participants), matching the two-validator devnet's own minimum-viable-group size.

**The sentinel fee token is `MyToken`, deployed as-is via the existing
`DeployERC20.s.sol`/`DeployERC20Script` — no contract changes — with its 18-decimal
default read as a dollar-pegged stablecoin, USDS-style, rather than USDC-style (6
decimals).** `MyToken` currently has no way to configure its `decimals()` (it inherits
OpenZeppelin `ERC20`'s unconfigurable 18-decimal default). Rather than changing that
(see Alternatives), the devnet's request fee is expressed directly in `MyToken`'s
existing 18 decimals: **40 cents ⇒ `400000000000000000` base units** (`0.40 * 10^18`).
This keeps the fee human-meaningful (a real dollar amount, matching the product intent)
with zero Solidity changes — the token is simply treated as 18-decimal "dollars" rather
than 6-decimal ones.

**Sentinel bonds/fees use that fee token and a freshly deployed `SentinelOracleV2`,
deployed alongside the existing `Consensus`.** `contracts/script/DeployERC20.s.sol` and
`contracts/script/DeploySentinelOracleV2.s.sol` already exist and are exercised by
`run_sentinel_integration_test.sh`; `run_devnet.sh` reuses them, pointing
`SENTINEL_CONSENSUS` at the real `Consensus` address the existing `DeployScript` produces
(`run_devnet.sh:90`) instead of `run_sentinel_integration_test.sh`'s `TestConsensus`
stand-in. `SentinelOracleV2`'s other parameters reuse `run_sentinel_integration_test.sh`'s
own existing devnet-appropriate values for everything except the fee itself
(`SENTINEL_REQUEST_FEE=400000000000000000` per the above, `SENTINEL_COMMIT_WINDOW=5`,
`SENTINEL_REVEAL_WINDOW=5`, `SENTINEL_GOVERNANCE_DELAY=0`, `SENTINEL_BOND_MULTIPLIER=2`
— so the bond target per commitment is `800000000000000000`, i.e. $0.80). Each
configured sentinel is registered via the oracle's `addSentinel(address)` and funded
with ETH (gas) and enough of the fee token to comfortably cover that bond target across
more than one request — e.g. `10000000000000000000` ($10) — rather than
`run_sentinel_integration_test.sh`'s own `FUNDING_TOKEN=1000000`, which is far too small
at this fee's 18-decimal scale. The sentinel service itself already emits the ERC-20
`approve` as an onchain action the moment it needs to lock a bond
(`SentinelActionKind::ApproveToken`, `crates/sentinel/src/service.rs:529`), so the devnet
script does not need to submit any approval itself.

### Alternatives Considered

- **Bundle the Rust binaries directly into the existing `contracts` image or a shared
  base image**, instead of one `Dockerfile` per crate. Rejected: `validator/Dockerfile`
  and `contracts/Dockerfile` already keep one image per logical service, and the Rust
  binaries have no dependency on the Foundry toolchain the `contracts` image bundles —
  a shared image would only make each image larger and its purpose less obvious.
- **Keep running the TypeScript validator in `run_devnet.sh` alongside the new Rust
  validator**, the way `run_validator_port_integration_test.sh` deliberately runs both
  side by side to test cross-implementation compatibility. Rejected for the devnet
  specifically: that script's whole point is exercising both implementations
  together; the devnet's purpose per this epic is to be the Rust-only, sentinel-inclusive
  local environment, and `run_integration_test.sh` already remains available unchanged
  for anyone who wants a TypeScript-only or mixed run (see Assumptions).
- **Have the devnet script call the Rust binaries directly on the host (`cargo run
  --package validator`) instead of containerizing them**, matching how
  `run_sentinel_integration_test.sh`/`run_validator_port_integration_test.sh` do it.
  Rejected: `run_devnet.sh`'s whole design (podman pod, `--build`, `podman kube play`)
  exists specifically to give a container-based devnet with no host toolchain
  requirement beyond `podman` itself; switching to bare `cargo run` here would be a
  bigger, unrelated redesign of the script, not a Rust migration of it.
- **Make `MyToken`'s decimals configurable, and deploy the devnet's fee token with 6
  decimals (USDC-style) to emulate a dollar-pegged stablecoin.** Rejected: it would pull
  a Solidity contract change into what is otherwise a script/Dockerfile-only epic. Reading
  the token's existing 18-decimal default as a stablecoin instead (USDS-style) gets the
  same "fees/bonds read as real dollar amounts" outcome with zero contract changes — see
  Architecture Decision.
- **Persist devnet validator/sentinel state to a host-mounted SQLite file instead of
  `sqlite::memory:`**, so state survives a `podman kube play` restart without redoing
  DKG/genesis. Rejected for now: this matches the TypeScript validator's own existing
  devnet default (`STORAGE_FILE` is unset by `run_devnet.sh` today, defaulting to
  `:memory:` per `validator/src/types/schemas.ts:100`) — i.e. it's not a behavior
  change — and can be revisited later without affecting this epic's shape if persistence
  turns out to matter in practice.

---

## Tech Specs

### Phase 1 — Add `Dockerfile`s for `crates/validator` and `crates/sentinel`

New files only; nothing else changes, and neither binary is wired into any script yet.

- `crates/validator/Dockerfile`: multi-stage build — a builder stage (`rust:1-slim` or
  equivalent) running `cargo build --release --package validator` against the full
  workspace, and a slim runtime stage (e.g. `debian:trixie-slim`, matching glibc-linked
  release binaries) copying out `target/release/validator` plus any shared libraries it
  needs (`ca-certificates` for TLS to the RPC endpoint and the sentinel's optional
  `remote_check_url` — check whether it's actually needed given
  `workspace.dependencies.reqwest` sets `default-features = false, features =
  ["json", "rustls"]`). `CMD` runs the binary directly (no entrypoint script needed —
  unlike `validator/bin/entrypoint.sh`, there is no `STORAGE_FILE`/`STORAGE_BACKUP`
  behavior to reproduce; devnet state is in-memory, per the Architecture Decision).
  This is the production image (see Architecture Decision) — no devnet-specific
  shortcuts.
- `crates/validator/Dockerfile.dockerignore`: allow-list `Cargo.toml`, `Cargo.lock`,
  `crates/**` only (see Architecture Decision).
- `crates/sentinel/Dockerfile` / `crates/sentinel/Dockerfile.dockerignore`: same shape,
  `--package sentinel`.
- Acceptance bar: `podman build -f crates/validator/Dockerfile -t localhost/safenet-validator .`
  and the `sentinel` equivalent both succeed from a clean checkout, and running the
  resulting image with `--version` prints the crate version (matches the `--version`
  switch both binaries already expose in `main.rs`).
- This phase can be one PR covering both Dockerfiles (they're small and fully
  symmetric), or split into two if the reviewer prefers one crate per PR.

### Phase 2 — Migrate `run_devnet.sh`'s validators to the Rust image

Depends on Phase 1.

- `podman build -t localhost/safenet-validator -f "$ROOT/validator/Dockerfile" "$ROOT"`
  (`run_devnet.sh:68`) becomes `-f "$ROOT/crates/validator/Dockerfile"`.
- A per-run temporary directory is created (e.g. `config_dir="$(mktemp -d)"`, cleaned up
  on exit via `trap`), and for each entry in `VALIDATORS` a TOML file is written into it,
  following the shape `run_validator_port_integration_test.sh`'s `$RUST_CFG` heredoc
  already establishes:
  ```toml
  rpc = "http://localhost:8545"
  signer = "<private_key>"
  database = "sqlite::memory:"

  [validator]
  consensus = "<consensus address>"
  blocks_per_epoch = <blocks_per_epoch>

  [[validator.participants]]
  address = "<participant address>"
  # ... one block per entry in $participants

  [index]
  block_time = <block_time * 1000>
  start_block = 0
  ```
  (`config.validator.oracles` is populated in Phase 3 once a `SentinelOracle` exists for
  the validator to honor attestations from.)
- `safenet_spec()`'s per-validator container (`run_devnet.sh:116-140`) drops its `env:`
  block entirely and gains a `volumeMounts`/`args` pair instead:
  ```yaml
      - name: validator-${name}
        image: localhost/safenet-validator:latest
        args:
          - --config-file=/config/validator.toml
        volumeMounts:
          - name: config-${name}
            mountPath: /config
  ```
  with a matching `volumes:` entry (`hostPath`, pointing at the generated file for that
  validator) added under `spec:`.
- `usage()`'s `--build` line (`run_devnet.sh:20`) is reworded to mention the Rust image
  instead of the TypeScript one.
- Acceptance bar: `npm run devnet -- --build` brings up Anvil plus both Rust validators,
  genesis completes, and the existing `contracts`/`DeployScript`/`GenesisScript` flow
  (`run_devnet.sh:85-165`) is otherwise untouched.

### Phase 3 — Spin up Rust sentinels

Depends on Phase 1 (sentinel `Dockerfile`) and Phase 2 (script already generates
per-instance TOML/volumes, extended here).

- A `SENTINELS` array, parallel in shape to `VALIDATORS` (`name:address:private_key`),
  added near the top of the script alongside it, using the `carol`/`dave` identities
  from the Architecture Decision table. A hardcoded `ARBITRATOR` address (the `(5)`
  identity from the same table) is added alongside it.
- Contract deployment gains, after the existing `forge_script DeployScript`
  (`run_devnet.sh:160`):
  - `forge_script DeployERC20` (via `contracts/script/DeployERC20.s.sol`, already used by
    `run_sentinel_integration_test.sh`, unmodified) for the sentinel fee token.
  - `forge_script DeploySentinelOracleV2` with `SENTINEL_ARBITRATOR="$ARBITRATOR"`,
    `SENTINEL_CONSENSUS` (the `$consensus` address already parsed at `run_devnet.sh:90`),
    `SENTINEL_FEE_TOKEN`, `SENTINEL_REQUEST_FEE=400000000000000000` (40 cents at the fee
    token's 18 decimals, per the Architecture Decision), and
    `SENTINEL_COMMIT_WINDOW`/`SENTINEL_REVEAL_WINDOW`/`SENTINEL_GOVERNANCE_DELAY`/
    `SENTINEL_BOND_MULTIPLIER` set to `run_sentinel_integration_test.sh`'s existing values
    (`5`/`5`/`0`/`2`).
  - `cast send ... addSentinel(address)` for each entry in `SENTINELS`, and funding each
    with ETH and `10000000000000000000` ($10) of the fee token — comfortably above the
    `800000000000000000` ($0.80) bond target across several requests — rather than
    `run_sentinel_integration_test.sh`'s own `FUNDING_TOKEN=1000000`, which is far too
    small at this fee's scale. Run via `forge_script`'s existing
    `podman exec ... safenet-node` pattern (`cast` is already present in the `contracts`
    image, being part of the Foundry toolchain it's built `FROM`) rather than requiring
    `cast` on the host.
- Each `SENTINELS` entry gets a generated TOML config (same mechanism as Phase 2's
  validators):
  ```toml
  rpc = "http://localhost:8545"
  signer = "<private_key>"
  database = "sqlite::memory:"
  oracle = "<sentinel oracle address>"
  consensus = "<consensus address>"

  [sentinel]
  fee_token = "<fee token address>"
  voting_window = 10  # commit_window + reveal_window
  blocklist = []
  address_poisoning_lookback_blocks = <devnet default>

  [index]
  block_time = <block_time * 1000>
  ```
- Each `VALIDATORS` entry's generated config (Phase 2) gains the sentinel oracle address
  under `[validator] oracles = ["<sentinel oracle address>"]`, so validators honor its
  attestations on oracle-checked transactions.
- `safenet_spec()` gains one `sentinel-${name}` container per `SENTINELS` entry,
  structurally identical to the validator containers added in Phase 2 (mounted config,
  `localhost/safenet-sentinel:latest` image).
- `usage()` and the script's own top-level doc comment updated to describe the devnet as
  running validators and sentinels.
- Acceptance bar: `npm run devnet -- --build` brings up both sentinels, both register
  onchain, and, given a proposed oracle-checked transaction (e.g. via
  `contracts/script/ProposeOracleTransaction.s.sol`, manually or in a short smoke test),
  vote and resolve it — mirroring `run_sentinel_integration_test.sh`'s existing
  assertions but against the real devnet `Consensus`/validator set instead of
  `TestConsensus`.

### Phase 4 — Docs pass

- `AGENTS.md`'s "Local devnet" section and `README.md` (if it documents `npm run
  devnet`) updated to describe the Rust-only, sentinel-inclusive devnet.
- No code changes.

### Phase 5 — Remove this plan

Delete `epics/2026_07_28_migrate_devnet_to_rust.md` once Phases 1-4 are merged.

---

## Implementation Phases

| Phase | Summary | Depends on | Own PR |
|---|---|---|---|
| 1 | Add `crates/validator/Dockerfile` and `crates/sentinel/Dockerfile` (+ `.dockerignore`s), production-shaped | — | ✅ (or split in two) |
| 2 | Migrate `run_devnet.sh`'s validators from the TypeScript image/env-var config to the Rust image/TOML config | 1 | ✅ |
| 3 | Extend `run_devnet.sh` to deploy a `SentinelOracleV2` + `MyToken` fee token and spin up two Rust sentinels (`carol`/`dave`) | 1, 2 | ✅ |
| 4 | Update `AGENTS.md`/`README.md` devnet docs | 3 | ✅ |
| 5 | Remove this plan | 4 | ✅ |

---

## Open Questions and Assumptions

No open questions remain — the below were raised during planning and have since been
resolved into the Architecture Decision and Tech Specs above; they're recorded here as
the resulting scope boundaries and defaults.

### Assumptions

- **No TypeScript code is removed or deprecated by this epic.** `validator/` (both the
  validator and sentinel TypeScript implementations) and its `Dockerfile` are left fully
  intact; this epic only changes what `run_devnet.sh` itself builds and runs.
- **A separate, later epic removes the TypeScript validator and sentinel entirely**
  (including `scripts/run_integration_test.sh`), and, as part of that same cleanup,
  collapses the oracle contracts down to a single version (there are currently `V1`/`V2`
  variants — see `DeploySentinelOracle.s.sol` vs. `DeploySentinelOracleV2.s.sol`). This
  epic does not depend on or block that one.
- **`.github/workflows/docker.yml` is unmodified.** It publishes
  `ghcr.io/safe-research/safenet-validator` from `validator/Dockerfile` (the TypeScript
  one) as a real release artifact on every push to `main` and every `v*` tag. Wiring the
  new Rust `Dockerfile`s into it (or an equivalent) to publish `crates/validator`/
  `crates/sentinel` images is deferred to a separate epic; this epic only ensures those
  Dockerfiles are already production-shaped so that follow-up is mechanical.
- **`localhost/safenet-validator` is safe to repoint at the new Rust `Dockerfile`.**
  Confirmed via `grep` that `run_devnet.sh` is its only consumer; no other script or
  workflow depends on that tag resolving to the TypeScript image.
- **`sqlite::memory:` devnet state (no persistence across a pod restart) is acceptable**
  and matches the TypeScript validator's own existing devnet default. Revisiting this
  (host-mounted persistent storage) is left to the future if it turns out to matter in
  practice — see Alternatives Considered.
- **The devnet's sentinel fee token stays `MyToken` at its current, unconfigurable
  18-decimal default — no Solidity changes in this epic.** Its fee/bond amounts are
  still dollar-denominated (40 cents / $0.80) by reading those 18 decimals as a
  USDS-style stablecoin rather than adding a 6-decimal, USDC-style configuration option.
  A genuinely configurable, 6-decimal devnet stablecoin contract remains possible future
  work if ever wanted, but isn't needed to get dollar-meaningful amounts today.
- **Sentinel bonding needs no script-side ERC-20 `approve`.** The sentinel service itself
  emits the approval as an onchain action when it needs to lock a bond
  (`SentinelActionKind::ApproveToken`, `crates/sentinel/src/service.rs:529`); the devnet
  script only needs to fund each sentinel account with ETH and the fee token.
- **Exact `Dockerfile` base images/caching strategy (e.g. whether to use `cargo-chef` for
  layer caching) are left to implementation/PR review**, so long as the result is a
  single, production-shaped image per crate rather than a devnet-specific variant.
