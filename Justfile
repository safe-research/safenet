# Repo-wide command runner.
# `contracts/` has no JavaScript of its own, so its commands shell out to
# `forge` directly rather than through a package.json front door.

set shell := ["bash", "-euo", "pipefail", "-c"]

# Configure the default container runtime to use for Just recipes.
docker := env("DOCKER", "docker")

# List available recipes.
default:
    @just --list

# Build every buildable package (contracts, explorer).
build:
    (cd contracts && forge build --force)
    npm --prefix explorer run build

# Lint/format-check every package: Solidity, Rust, and (via each package's own check script) Biome/TypeScript.
check:
    (cd contracts && test "$(forge --version | head -1)" = "forge Version: 1.5.1-v1.5.1" && forge fmt --check && forge lint --deny notes)
    npm --prefix examples run check
    npm --prefix explorer run check
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    @just lint-openapi crates/sentinel-engine/openapi.yaml

# Auto-fix formatting issues.
fix:
    (cd contracts && forge fmt)
    npm --prefix examples run fix
    npm --prefix explorer run fix
    cargo fmt --all

# Run every package's unit tests.
test:
    (cd contracts && forge test -vvv)
    npm --prefix explorer run test
    cargo test --workspace

# Generate per-package coverage reports.
coverage:
    (cd contracts && mkdir -p coverage && FOUNDRY_PROFILE=coverage forge coverage --report lcov --report-file coverage/lcov.info)
    npm --prefix examples run coverage
    npm --prefix explorer run coverage
    cargo llvm-cov --workspace --no-report

# Generate an HTML coverage report (with branch coverage) from the merged
# lcov.info produced by scripts/generate_coverage_report.sh, and print its
# path. `@vitest/coverage-v8`'s lcov output trips a few of genhtml 2.x's
# stricter consistency checks (lcov 1.x doesn't perform them); genhtml <2
# doesn't recognize those category names and hard-errors if passed, so only
# apply them on genhtml >=2.
coverage-html:
    #!/usr/bin/env bash
    set -euo pipefail
    ./scripts/generate_coverage_report.sh
    genhtml_major_version="$(genhtml --version | grep -oE '[0-9]+' | head -1)"
    ignore_errors=()
    if [ "$genhtml_major_version" -ge 2 ]; then
        ignore_errors=(--ignore-errors inconsistent,corrupt,category)
    fi
    genhtml lcov.info "${ignore_errors[@]}" --branch-coverage --output-directory coverage/html
    echo "📄 Open coverage/html/index.html to view the report."

# Install the pinned Foundry toolchain version.
foundryup:
    foundryup --install v1.5.1

# Install NPM dependencies.
deps:
    npm --prefix examples ci
    npm --prefix explorer ci

# Install tooling required by `just coverage` on top of `deps`: cargo-llvm-cov
# and its llvm-tools-preview component.
coverage-deps: deps
    cargo install cargo-llvm-cov --locked
    rustup component add llvm-tools-preview

# Lints an OpenAPI specification file.
lint-openapi spec:
    {{docker}} run --rm \
        -v $PWD/{{spec}}:/spec/openapi.yaml:ro,z \
        ghcr.io/redocly/cli:2.46.0 lint --extends=spec openapi.yaml

# Start the local Podman devnet. Pass through any run_devnet.sh flag, e.g.
# `just devnet --build`.
devnet *args:
    ./scripts/run_devnet.sh {{args}}

# Rust sentinel bash integration test (Anvil + two sentinel instances).
test-integration-sentinel:
    ./scripts/run_sentinel_integration_test.sh

# Rust sentinel engine integration test (requires an external test-vector checkout).
test-integration-sentinel-engine test-vectors:
    TEST_VECTORS="{{test-vectors}}" ./scripts/run_sentinel_engine_integration_test.sh

# Rust validator bash integration test (Anvil + two validator instances).
test-integration-validator:
    ./scripts/run_validator_integration_test.sh

# Regression test: nonces must survive a reorg that rewinds a group's DKG
# past its key-share confirmation (Anvil + two validator instances).
test-integration-validator-reorg-nonce:
    ./scripts/run_validator_reorg_nonce_test.sh

# Run the explorer's Vite dev server.
explorer-dev:
    npm --prefix explorer run dev

# Run examples/attest-safe-tx.ts, e.g. `just examples-attest-safe-tx <safeTxHash> <guardAddress>`.
examples-attest-safe-tx *args:
    npm --prefix examples run attest-safe-tx -- {{args}}

# --- contracts/script/*.s.sol front doors (see contracts/script/README.md) ---

contracts-deploy *args:
    (cd contracts && forge script DeployScript {{args}})

contracts-genesis *args:
    (cd contracts && forge script GenesisScript {{args}})

contracts-deploy-erc20 *args:
    (cd contracts && forge script DeployERC20Script {{args}})

contracts-deploy-sentinel-oracle *args:
    (cd contracts && forge script DeploySentinelOracleScript {{args}})

contracts-deploy-test-consensus *args:
    (cd contracts && forge script DeployTestConsensusScript {{args}})

contracts-propose *args:
    (cd contracts && forge script ProposeTransactionScript {{args}})

contracts-deploy-staking *args:
    (cd contracts && forge script DeployStakingScript {{args}})

contracts-deploy-staking-tx-builder *args:
    (cd contracts && forge script DeployStakingWithTxBuilderScript {{args}})

contracts-propose-validators *args:
    (cd contracts && forge script ProposeValidatorsScript {{args}})

contracts-accept-validators *args:
    (cd contracts && forge script AcceptValidatorsScript {{args}})

contracts-stake-safe *args:
    (cd contracts && forge script StakeSafeScript {{args}})

contracts-initiate-withdraw *args:
    (cd contracts && forge script InitiateWithdrawScript {{args}})

contracts-claim-withdraw *args:
    (cd contracts && forge script ClaimWithdrawScript {{args}})
