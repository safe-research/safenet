#!/usr/bin/env bash
# CA3-FIX (2026-10-09): F2-XC-001 binary-level variants the run-3 re-validation did not list.
# Same route as run3/run.sh: the shipped binaries, a config derived from the shipped sample,
# stderr captured and grepped for 32 hex digits of a fake marker key.
#
# usage: ca3fix_run.sh <bin-dir> <work-dir>
set -u
BIN=${1:?bin dir}
WORK=${2:?work dir}
REPO=${REPO:-$(git -C "$(dirname "$0")" rev-parse --show-toplevel)}
MARKER=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
NEEDLE=0123456789abcdef0123456789abcdef
TOKEN=SECRETTOKEN0123456789
mkdir -p "$WORK"

base() {
  grep -v '^[[:space:]]*#' "$1" | grep -v '^[[:space:]]*$' |
    sed -e 's|^rpc = .*|rpc = "http://127.0.0.1:9"|' \
        -e "s|^signer = .*|signer = \"0x$MARKER\"|" \
        -e 's|^database = .*|database = "sqlite::memory:"|'
}
base "$REPO/crates/validator/validator.sample.toml" > "$WORK/validator-base.toml"
base "$REPO/crates/sentinel/sentinel.sample.toml" > "$WORK/sentinel-base.toml"

run() { # $1 label, $2 bin, $3 config path
  local out="$WORK/$1.stderr" rc
  timeout 20 "$BIN/$2" --config-file "$3" 2> "$out" > "$WORK/$1.stdout"; rc=$?
  echo "=================== $1 ($2) exit=$rc"
  head -c 400 "$out" | sed 's/^/    /'; echo
  echo "    marker-hex=$(grep -c "$NEEDLE" "$out") token=$(grep -c "$TOKEN" "$out") input-Some=$(grep -c 'input: Some' "$out")"
}

# w01: a config file that is not UTF-8 (a Latin-1 byte inside a comment line, as an editor on
# another locale might save it). `fs::read_to_string` fails before any parsing.
{ printf '# Configuraci\xf3n\n'; cat "$WORK/validator-base.toml"; } > "$WORK/w01-latin1.toml"
run w01-latin1 validator "$WORK/w01-latin1.toml"

# w02: the config saved as UTF-16 with a byte-order mark.
iconv -f UTF-8 -t UTF-16 "$WORK/validator-base.toml" > "$WORK/w02-utf16.toml"
run w02-utf16 validator "$WORK/w02-utf16.toml"

# w03: `--config-file` pointing at a directory, and at a missing file.
mkdir -p "$WORK/w03-dir.toml"
run w03-directory validator "$WORK/w03-dir.toml"
run w03-missing validator "$WORK/does-not-exist.toml"

# w04: the signer key pasted into an integer field the sample does not list
# (`blocks_per_epoch`, a NonZeroU64), quoted as a string.
sed -e "s|^consensus = \(.*\)|consensus = \1\nblocks_per_epoch = \"0x$MARKER\"|" "$WORK/validator-base.toml" > "$WORK/w04-key-in-integer.toml"
run w04-key-in-integer validator "$WORK/w04-key-in-integer.toml"

# w05: the sentinel engine URL with an API token but no scheme (`engine` is a plain `Url`).
sed -e "s|^engine = .*|engine = \"engine.internal:5473/check?token=$TOKEN\"|" "$WORK/sentinel-base.toml" > "$WORK/w05-engine-url-no-scheme.toml"
run w05-engine-url-no-scheme sentinel "$WORK/w05-engine-url-no-scheme.toml"

# w06: the signer key pasted into the sentinel `voting_window` (u64).
sed -e "s|^voting_window = .*|voting_window = \"0x$MARKER\"|" "$WORK/sentinel-base.toml" > "$WORK/w06-key-in-voting-window.toml"
run w06-key-in-voting-window sentinel "$WORK/w06-key-in-voting-window.toml"

# w07: the same mistake in `rpc` (env-substituted `Url`), for contrast with w05.
sed -e "s|^rpc = .*|rpc = \"rpc.internal:8545/v3/$TOKEN\"|" "$WORK/sentinel-base.toml" > "$WORK/w07-rpc-url-no-scheme.toml"
run w07-rpc-url-no-scheme sentinel "$WORK/w07-rpc-url-no-scheme.toml"
echo "# done"
