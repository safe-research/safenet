#!/usr/bin/env bash
# rv-xc run 3 re-run of F2-XC-002: the shipped samples' bare `sqlite:<path>` database URL cannot be
# opened on a fresh (empty) volume; `?mode=rwc` can. Same route as QA2-XC: the real binaries, the
# loopback JSON-RPC stub `../stub.py` so that `main` gets past `Provider::connect`.
#
# usage: run.sh <bin-dir> <scratch-dir> <port>
#   configs are the current samples (comment lines dropped) with `rpc` pointed at the stub and
#   `database` pointed into <scratch-dir>/vol-*, an existing empty directory per variant.
set -u
BIN=${1:?bin dir}
SCRATCH=${2:?scratch dir}
PORT=${3:?stub port}
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(git -C "$HERE" rev-parse --show-toplevel)}
mkdir -p "$SCRATCH"

python3 -I "$HERE/../stub.py" "$PORT" & STUB=$!
trap 'kill "$STUB" 2>/dev/null; wait "$STUB" 2>/dev/null' EXIT
for _ in $(seq 1 50); do
  curl -s -m 1 -X POST -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' "http://127.0.0.1:$PORT" > /dev/null && break
  sleep 0.1
done
echo "# stub pid $STUB on 127.0.0.1:$PORT (answers every call with 0x64); listener:"
ss -ltnp 2>/dev/null | grep ":$PORT " | sed 's/^/#   /'

cfg() { # $1 sample, $2 database value, $3 output
  grep -v '^[[:space:]]*#' "$1" | grep -v '^[[:space:]]*$' |
    sed -e "s|^rpc = .*|rpc = \"http://127.0.0.1:$PORT\"|" \
        -e 's|^signer = .*|signer = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"|' \
        -e "s|^database = .*|database = \"$2\"|" > "$3"
}

run() { # $1 name, $2 binary, $3 sample, $4 database value, [$5 env]
  local vol="$SCRATCH/vol-$1"
  mkdir -p "$vol"
  cfg "$3" "$4" "$SCRATCH/$1.toml"
  echo "=================== $1 ($2)"
  echo "--- database line: $(grep '^database' "$SCRATCH/$1.toml" | sed "s|$SCRATCH|<scratch>|")"
  [ -n "${5:-}" ] && echo "--- environment: $(echo "$5" | sed "s|$SCRATCH|<scratch>|")"
  echo "--- ls vol before: $(ls "$vol")"
  env ${5:-} timeout 20 "$BIN/$2" --config-file "$SCRATCH/$1.toml" 2>&1 >/dev/null | sed "s|$SCRATCH|<scratch>|g"
  echo "--- exit=${PIPESTATUS[0]}"
  echo "--- ls vol after: $(ls "$vol")"
}

V="$REPO/crates/validator/validator.sample.toml"
S="$REPO/crates/sentinel/sentinel.sample.toml"
echo "# sample database lines: $(grep -h '^database' "$V" "$S" | tr '\n' ' ')"
run validator-bare     validator "$V" "sqlite:$SCRATCH/vol-validator-bare/storage.db"
run validator-rwc      validator "$V" "sqlite:$SCRATCH/vol-validator-rwc/storage.db?mode=rwc"
run sentinel-bare      sentinel  "$S" "sqlite:$SCRATCH/vol-sentinel-bare/storage.db"
run sentinel-rwc       sentinel  "$S" "sqlite:$SCRATCH/vol-sentinel-rwc/storage.db?mode=rwc"
run validator-env-bare validator "$V" '${RV_XC_DB}' "RV_XC_DB=sqlite:$SCRATCH/vol-validator-env-bare/storage.db"
echo "# done"
