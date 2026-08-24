#!/bin/bash
# Runs sentinel engine test specs against a running sentinel engine, purely
# over curl/HTTP. Because it only speaks the API in
# crates/sentinel-engine/openapi.yaml, the exact same script and specs can
# validate any sentinel engine implementation, not just this repo's Rust one
# — set SENTINEL_ENGINE_URL to point it at an already-running engine instead
# of having this script build and start the Rust one.
#
# Specs follow the `specs/<group>/<case>.json` format defined by
# https://github.com/safe-research/sentinel-test-vectors (see its
# "Spec file format" docs): a `SafeTransaction` plus its expected verdict —
#   { "transaction": {...}, "verdict": "insecure", "rule": "R-4.1", "note": "..." }
# `rule` is present only when `verdict` is `"insecure"`; `note` and `txHash`
# are optional. `"abstain"` is never a spec's expected verdict — it's an
# engine behavior, reported below as SKIP rather than PASS/FAIL.
#
# These specs currently live in crates/sentinel-engine/specs, but are
# expected to move to the sentinel-test-vectors repository above; set
# SENTINEL_ENGINE_SPECS_DIR to a checkout of it (or its `specs/` directory)
# once that exists.
set -eo pipefail
# Job control, so the Anvil/engine background jobs started below each get
# their own process group (see cleanup()).
set -m

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

SPECS_DIR="${SENTINEL_ENGINE_SPECS_DIR:-$ROOT/crates/sentinel-engine/specs}"
RPC_URL="http://127.0.0.1:8545"
OWN_ENGINE_ADDR="127.0.0.1:5473"

PIDS=()
cleanup() {
	for pid in "${PIDS[@]}"; do
		# Negative PID targets the whole process group `set -m` gave this
		# job, so any subprocesses are reaped too rather than left orphaned
		# holding a port.
		kill -- "-$pid" >/dev/null 2>&1 || true
	done
	rm -f "${ENGINE_CONFIG:-}"
}
trap cleanup EXIT

if [ -n "${SENTINEL_ENGINE_URL:-}" ]; then
	ENGINE_URL="$SENTINEL_ENGINE_URL"
	echo "Testing the already-running sentinel engine at $ENGINE_URL..."
else
	ENGINE_URL="http://$OWN_ENGINE_ADDR"

	echo "Building the Rust sentinel engine..."
	cargo build --package sentinel-engine

	echo "Starting Anvil..."
	anvil >"$ROOT/anvil_logs.txt" 2>&1 &
	PIDS+=("$!")
	sleep 2

	ENGINE_CONFIG=$(mktemp)
	cat >"$ENGINE_CONFIG" <<EOF
rpc = "$RPC_URL"
bind_address = "$OWN_ENGINE_ADDR"

[engine]
blocklist = []
address_poisoning_lookback_blocks = 1000
EOF

	echo "Starting the sentinel engine..."
	"$ROOT/target/debug/sentinel-engine" --config-file "$ENGINE_CONFIG" >"$ROOT/sentinel_engine_logs.txt" 2>&1 &
	PIDS+=("$!")

	echo "Waiting for the sentinel engine to come up..."
	up=0
	for _ in $(seq 1 50); do
		if curl -s -o /dev/null "$ENGINE_URL"; then
			up=1
			break
		fi
		sleep 0.2
	done
	if [ "$up" -ne 1 ]; then
		echo "Sentinel engine never came up; see $ROOT/sentinel_engine_logs.txt" >&2
		exit 1
	fi
fi

shopt -s nullglob
SPEC_FILES=("$SPECS_DIR"/*/*.json)
shopt -u nullglob
if [ ${#SPEC_FILES[@]} -eq 0 ]; then
	echo "No spec files found in $SPECS_DIR" >&2
	exit 1
fi

echo "Running ${#SPEC_FILES[@]} spec(s) against $ENGINE_URL..."
PASSED=0
FAILED=0
SKIPPED=0
for file in "${SPEC_FILES[@]}"; do
	name=${file#"$SPECS_DIR"/}
	# The wire request is just the transaction; `{transaction}` is jq's
	# object-construction shorthand for `{transaction: .transaction}`.
	request=$(jq -c '{transaction}' "$file")
	expected=$(jq -S -c 'if .verdict == "insecure" then {rule, verdict} else {verdict} end' "$file")

	body=$(curl -s -X POST "$ENGINE_URL/v1/security-check" \
		-H 'content-type: application/json' \
		-d "$request")
	# Falls back to the raw body if it isn't valid JSON (e.g. an error page),
	# so a malformed response still shows a readable diff below instead of
	# aborting the whole run.
	actual=$(printf '%s' "$body" | jq -S -c '.' 2>/dev/null || printf '%s' "$body")
	verdict=$(printf '%s' "$body" | jq -r '.verdict // empty' 2>/dev/null || true)

	if [ "$verdict" = "abstain" ]; then
		echo "  SKIP  $name: engine abstained"
		SKIPPED=$((SKIPPED + 1))
	elif [ "$actual" = "$expected" ]; then
		echo "  PASS  $name"
		PASSED=$((PASSED + 1))
	else
		echo "  FAIL  $name: expected $expected, got $actual"
		FAILED=$((FAILED + 1))
	fi
done

echo "$PASSED passed, $FAILED failed, $SKIPPED skipped (of ${#SPEC_FILES[@]})"
if [ "$FAILED" -ne 0 ] || [ "$SKIPPED" -ne 0 ]; then
	exit 1
fi
