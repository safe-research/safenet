#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Collect coverage data for the whole workspace in a single test run.
cargo llvm-cov --workspace --no-report

# Emit a per-crate LCOV report so that each crate shows up as its own row in
# the merged coverage report. Source paths are rewritten to be relative to the
# repository root to match the convention used by the TypeScript and Solidity
# reports (i.e. "SF:crates/core/src/...").
for manifest in crates/*/Cargo.toml; do
    crate="$(dirname "$manifest")"
    name="$(grep -m1 '^name' "$manifest" | cut -d '"' -f2)"
    lcov="$crate/coverage/lcov.info"

    mkdir -p "$crate/coverage"
    cargo llvm-cov report --package "$name" --lcov --output-path "$lcov"
    sed -i "s|^SF:$ROOT/|SF:|" "$lcov"
done
