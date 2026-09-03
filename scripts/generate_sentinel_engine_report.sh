#!/usr/bin/env bash

# Runs the sentinel engine integration tests and writes a PR-comment-ready
# markdown report (sentinel-engine-report.md) summarizing the results.
#
# Usage: scripts/generate_sentinel_engine_report.sh <path-to-sentinel-test-vectors>

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TEST_VECTORS="$1"

OUTPUT_FILE="$(mktemp)"
trap 'rm -f "$OUTPUT_FILE"' EXIT

# `just test-integration-sentinel-engine` prints one PASS/FAIL/SKIP line per
# spec, followed by a per-group scorecard table (see
# sentinel-test-vectors' bin/run-tests.sh). Capture it once so both the
# always-visible summary and the collapsed per-spec detail below come from a
# single run.
just test-integration-sentinel-engine "$TEST_VECTORS" 2>&1 | tee "$OUTPUT_FILE"

CASES=$(grep -E '^(PASS|FAIL|SKIP) specs/' "$OUTPUT_FILE" || true)
SCORECARD=$(awk '/^GROUP /{found=1} found' "$OUTPUT_FILE")

{
    echo "<!-- sentinel-engine-report -->"
    echo "### 🛡️ Sentinel Engine Integration Test Results"
    echo ""
    echo '```text'
    echo "$SCORECARD"
    echo '```'
    echo ""
    echo "<details>"
    echo "<summary><strong>📄 Click to view individual test results</strong></summary>"
    echo ""
    echo '```text'
    echo "$CASES"
    echo '```'
    echo ""
    echo "</details>"
} > sentinel-engine-report.md

echo "📝 Wrote sentinel-engine-report.md"
