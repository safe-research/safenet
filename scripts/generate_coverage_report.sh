#!/usr/bin/env bash

# Generates a merged test coverage report (coverage-report.md) across all
# JavaScript/TypeScript, Solidity and Rust packages in the repository.
#
# It assumes that all required tools are already available on PATH: node/npm,
# just, foundry (forge), lcov, jq and cargo-llvm-cov (with the llvm-tools-preview
# component). See the install-lcov and install-cargo-llvm-cov actions for how
# CI provisions these.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# --- 1. Generate coverage data --------------------------------------------

# JavaScript/TypeScript and Solidity coverage, emitted per package as
# {package}/coverage/lcov.info. Rust coverage data is collected for the whole
# workspace in a single test run, with no per-crate report yet.
echo "🧪 Generating coverage..."
just coverage

# --- 2. Build the report --------------------------------------------------

MERGE_ARGS=()
REPORT_TABLE=("| Package | Coverage |" "| :--- | :--- |")

# `@vitest/coverage-v8`'s lcov output can have a branch-coverage record with
# no corresponding line-coverage record for the same location, which lcov 2.x
# treats as a hard "inconsistent"/"corrupt" error when reading it back (lcov
# 1.x doesn't perform this check). Downgrade both to warnings on every lcov
# invocation below that reads a tracefile.
LCOV_IGNORE_ERRORS=(--ignore-errors inconsistent,corrupt)

# Process a single LCOV tracefile: collect per-package stats, add a table row
# and queue it for the final merge.
# Usage: process_lcov <lcov-file> <display-name>
process_lcov() {
    local lcov_file="$1"
    local display_name="$2"

    # Run lcov summary on this specific file and extract the percentage.
    local stats=$(lcov "${LCOV_IGNORE_ERRORS[@]}" --summary "$lcov_file" | grep "lines......" | cut -d ':' -f 2 | xargs)

    echo "   📊 $display_name: $stats"
    lcov "${LCOV_IGNORE_ERRORS[@]}" --list "$lcov_file"

    # Add to the merge list and append row to the table.
    MERGE_ARGS+=(--add-tracefile "$lcov_file")
    REPORT_TABLE+=("| **${display_name}** | ${stats} |")
}

# Scan NPM packages ("*/package.json").
echo "🔍 Scanning NPM packages..."
for manifest in */package.json; do
    [ -f "$manifest" ] || continue

    pkg="$(dirname "$manifest")"
    lcov_file="$pkg/coverage/lcov.info"

    # Filter out packages with no LCOV data, since not all packages generate
    # them (notably `example`).
    [ -f "$lcov_file" ] || continue

    # NPM tools emit source paths relative to the package directory. Prepend
    # the package path so they resolve from the repository root.
    sed -i "s|^SF:|SF:$pkg/|g" "$lcov_file"

    process_lcov "$lcov_file" "$pkg"
done

# Scan the Solidity contracts package. It has no package.json (see Justfile),
# so the NPM scan above never picks it up even though `just coverage` (run
# above) does generate its LCOV data.
echo "🔍 Scanning Solidity contracts..."
lcov_file="contracts/coverage/lcov.info"
if [ -f "$lcov_file" ]; then
    # forge emits source paths relative to contracts/, same as the NPM
    # packages above.
    sed -i "s|^SF:|SF:contracts/|g" "$lcov_file"
    process_lcov "$lcov_file" "contracts"
fi

# Scan Rust crates (crates/*).
echo "🔍 Scanning Rust workspace crates..."
for manifest in crates/*/Cargo.toml; do
    [ -f "$manifest" ] || continue

    crate="$(dirname "$manifest")"
    name="$(grep -m1 '^name' "$manifest" | cut -d '"' -f 2)"
    lcov_file="$crate/coverage/lcov.info"

    # `cargo llvm-cov` generates a report for all packages in the workspace,
    # so extract the LCOV information for the specific crate for the report.
    # Note that we also re-write absolute paths into relative ones to match
    # the format for TypeScript and Solidity.
    mkdir -p "$crate/coverage"
    cargo llvm-cov report --package "$name" --lcov --output-path "$lcov_file"
    sed -i "s|^SF:$ROOT/|SF:|" "$lcov_file"

    process_lcov "$lcov_file" "$crate"
done

# Merge all reports into a single tracefile. Enabling branch coverage is
# required or lcov silently drops all BRDA (branch) records during the merge.
# The RC option for this was renamed from `lcov_branch_coverage` to
# `branch_coverage` in lcov 2.0, and lcov treats the old name as a hard error
# rather than a warning, so pick the spelling that matches the installed
# version instead of hard-coding one.
LCOV_MAJOR_VERSION="$(lcov --version | grep -oE '[0-9]+' | head -1)"
if [ "$LCOV_MAJOR_VERSION" -ge 2 ]; then
    BRANCH_COVERAGE_RC="branch_coverage=1"
else
    BRANCH_COVERAGE_RC="lcov_branch_coverage=1"
fi

echo "🔗 Merging all reports..."
lcov "${LCOV_IGNORE_ERRORS[@]}" --rc "$BRANCH_COVERAGE_RC" "${MERGE_ARGS[@]}" --output-file lcov.info

# Total summary line coverage (e.g. "85.4%").
TOTAL_STATS=$(lcov "${LCOV_IGNORE_ERRORS[@]}" --summary lcov.info | grep "lines......" | cut -d ':' -f 2 | xargs)

# Per-file list. grep -v removes the "Reading tracefile" noise.
FILE_LIST=$(lcov "${LCOV_IGNORE_ERRORS[@]}" --list lcov.info | grep -v "Reading tracefile")

# Write the report file. Variables are used directly here (no template
# interpolation) to avoid URL-encoding of newlines and unintended shell
# expansion in heredocs.
{
    echo "<!-- coverage-report -->"
    echo "### 🛡️ Test Coverage Report"
    echo ""
    echo "**Total Line Coverage:** ${TOTAL_STATS}"
    echo ""
    printf '%s\n' "${REPORT_TABLE[@]}"
    echo ""
    echo "<details>"
    echo "<summary><strong>📄 Click to view coverage for all files</strong></summary>"
    echo ""
    echo '```text'
    echo "${FILE_LIST}"
    echo '```'
    echo ""
    echo "</details>"
} > coverage-report.md

echo "📝 Wrote coverage-report.md"
