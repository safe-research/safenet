#!/usr/bin/env bash
# Run 3 re-validation of F2-XC-001 (rv-xc): does a configuration parse error still print the
# configuration file, signer key included? Same intended route as QA2-XC and QA2-VAL-B: the
# shipped binaries, a copy of the shipped sample with one operator mistake, stderr captured.
#
# usage: run.sh <bin-dir> <work-dir>
#   <bin-dir>  directory holding the `validator` and `sentinel` debug binaries
#   <work-dir> empty scratch directory for the generated configs
#
# Every config is the current sample (comment lines dropped) with:
#   rpc      = "http://127.0.0.1:9"   loopback discard port; parsing fails before any connection
#   database = "sqlite::memory:"
#   signer   = MARKER                 a fake but valid secp256k1 scalar, easy to grep for
# plus exactly one mistake per variant. The script greps stderr for 32 hex digits of the marker,
# for the RPC host and for two document-only strings (`log_filter`, `genesis_salt` or `voting_window`).
set -u
BIN=${1:?bin dir}
WORK=${2:?work dir}
REPO=${REPO:-$(git -C "$(dirname "$0")" rev-parse --show-toplevel)}  # checkout whose samples are copied
MARKER=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
NEEDLE=0123456789abcdef0123456789abcdef
mkdir -p "$WORK"

base() { # $1 = sample path
  grep -v '^[[:space:]]*#' "$1" | grep -v '^[[:space:]]*$' |
    sed -e 's|^rpc = .*|rpc = "http://127.0.0.1:9"|' \
        -e "s|^signer = .*|signer = \"0x$MARKER\"|" \
        -e 's|^database = .*|database = "sqlite::memory:"|'
}

VSAMPLE="$REPO/crates/validator/validator.sample.toml"
SSAMPLE="$REPO/crates/sentinel/sentinel.sample.toml"
base "$VSAMPLE" > "$WORK/validator-base.toml"
base "$SSAMPLE" > "$WORK/sentinel-base.toml"

# variant name | binary | how the config is derived (sed program applied to the base) | env
mk() { # $1 name, $2 bin, $3 sed program
  local base_file="$WORK/$2-base.toml"
  sed -e "$3" "$base_file" > "$WORK/$1.toml"
  cmp -s "$base_file" "$WORK/$1.toml" && { echo "!! variant $1: sed changed nothing"; exit 2; }
}

mk v01-typo-table           validator 's|^\[transactions\]|[transaction]|'
mk v02-typo-key-sentinel    sentinel  's|^engine = |engin = |'
mk v03-typo-key-validator   validator 's|^consensus = \(.*\)|consensus = \1\nblocks_per_epoc = 1440|'
mk v04-signer-odd-digits    validator "s|^signer = .*|signer = \"0x${MARKER:1}\"|"
mk v05-signer-31-bytes      validator "s|^signer = .*|signer = \"0x${MARKER:2}\"|"
mk v06a-signing-timeout-0   validator 's|^consensus = \(.*\)|consensus = \1\nsigning_timeout = 0|'
mk v06b-blocks-per-epoch-0  validator 's|^consensus = \(.*\)|consensus = \1\nblocks_per_epoch = 0|'
mk v07a-unknown-top-level   validator 's|^database = \(.*\)|database = \1\nnot_a_real_field = "typo"|'
mk v07b-unknown-in-index    validator 's|^\[index\]|[index]\nuse_client_filterring = true|'
mk v08-signer-unterminated  sentinel  "s|^signer = .*|signer = \"0x$MARKER|"
mk v09-signer-unquoted-hex  validator "s|^signer = .*|signer = 0x$MARKER|"
mk v10-signer-bare          validator "s|^signer = .*|signer = $MARKER|"
mk v12-signer-array         validator "s|^signer = .*|signer = [\"0x$MARKER\"]|"
mk v13-signer-duplicate     validator "s|^signer = \(.*\)|signer = \1\nsigner = \1|"
mk v14-wrong-type-flattened sentinel  's|^\[transactions\]|[transactions]\nmax_in_flight_transactions = "16"|'
mk v15-signer-bad-hex-char  validator "s|^signer = .*|signer = \"0x${MARKER:0:63}g\"|"
# environment-variable route (#1007): the file names the variable, the value is malformed
mk v11a-env-signer-malformed validator 's|^signer = .*|signer = "${RV_XC_SIGNER}"|'
mk v11b-env-signer-unset     validator 's|^signer = .*|signer = "${RV_XC_SIGNER_UNSET}"|'
# control: the base config parses, so the run proceeds past `Config::load` (to the RPC step)
cp "$WORK/validator-base.toml" "$WORK/c01-control-parses.toml"

run() { # $1 variant, $2 bin, [$3 env assignment]
  local cfg="$WORK/$1.toml" out="$WORK/$1.stderr" rc
  echo "=================== $1 ($2)"
  echo "--- the lines that differ from the base config:"
  diff "$WORK/$2-base.toml" "$cfg" | grep '^[<>]' | sed 's/^/    /'
  if [ -n "${3:-}" ]; then echo "--- environment: ${3%%=*}=<value, see variant name>"; fi
  # stderr only, as in run 2 (`2>&1 1>/dev/null`); stdout is checked separately below
  env ${3:-} timeout 20 "$BIN/$2" --config-file "$cfg" 2> "$out" > "$WORK/$1.stdout"; rc=$?
  echo "--- exit=$rc   stderr bytes=$(wc -c < "$out")   stdout bytes=$(wc -c < "$WORK/$1.stdout")"
  echo "--- stderr (first 600 characters):"
  head -c 600 "$out" | sed 's/^/    /'; echo
  echo "--- grep counts on stderr: marker-hex=$(grep -c "$NEEDLE" "$out")" \
       "rpc-host=$(grep -c '127.0.0.1:9' "$out")" \
       "doc-only-strings=$(grep -c 'log_filter\|genesis_salt\|voting_window' "$out")" \
       "input-Some=$(grep -c 'input: Some' "$out")"
}

echo "# F2-XC-001 run 3 re-validation; binaries: $BIN"
echo "# validator: $("$BIN/validator" --version)  sentinel: $("$BIN/sentinel" --version)"
echo "# marker key 0x$MARKER (fake, valid scalar); grep needle $NEEDLE"
for v in v01-typo-table v03-typo-key-validator v04-signer-odd-digits v05-signer-31-bytes \
         v06a-signing-timeout-0 v06b-blocks-per-epoch-0 v07a-unknown-top-level v07b-unknown-in-index \
         v09-signer-unquoted-hex v10-signer-bare v12-signer-array v13-signer-duplicate \
         v15-signer-bad-hex-char; do
  run "$v" validator
done
for v in v02-typo-key-sentinel v08-signer-unterminated v14-wrong-type-flattened; do
  run "$v" sentinel
done
run v11a-env-signer-malformed validator "RV_XC_SIGNER=0x${MARKER:1}"
run v11b-env-signer-unset validator
run c01-control-parses validator
echo "# done"
