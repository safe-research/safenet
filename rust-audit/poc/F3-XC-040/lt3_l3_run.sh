#!/usr/bin/env bash
# LT3 (2026-10-09): independent run for lead CA3-FIX-L3 at 361aaf3.
# Usage: lt3_l3_run.sh <dir-with-validator-and-sentinel-binaries> <scratch-dir>
# A fake, clearly marked key (never a real one) is pasted into typed fields;
# stderr is searched for it. The signer field holds the sample's placeholder.
set -u
BIN="$1"; S="$2"; mkdir -p "$S"
MARK=feedfacecafebeeffeedfacecafebeeffeedfacecafebeeffeedfacecafebeef
PH=0x0000000000000000000000000000000000000000000000000000000000000001
val() { cat <<EOF
rpc = "http://127.0.0.1:9"
signer = "$PH"
database = "sqlite::memory:"
[validator]
consensus = "0x0000000000000000000000000000000000000001"
genesis_salt = "0x0000000000000000000000000000000000000000000000000000000000000000"
$1
[[validator.participants]]
address = "0x0000000000000000000000000000000000000001"
[[validator.participants]]
address = "0x0000000000000000000000000000000000000002"
[observability]
log_filter = "info"
EOF
}
sen() { cat <<EOF
rpc = "http://127.0.0.1:9"
signer = "$PH"
database = "sqlite::memory:"
oracle = "0x0000000000000000000000000000000000000001"
consensus = "0x0000000000000000000000000000000000000002"
[sentinel]
fee_token = "0x0000000000000000000000000000000000000003"
voting_window = $1
engine = $2
[observability]
log_filter = "info"
EOF
}
run() { # name binary config
  printf '%s\n' "$3" > "$S/$1.toml"
  out=$(timeout 20 "$BIN/$2" --config-file "$S/$1.toml" 2>&1); rc=$?
  hits=$(printf '%s' "$out" | grep -o "feedfacecafebeef" | wc -l)
  echo "=================== $1 ($2) exit=$rc marker_hits=$hits"
  printf '%s\n' "$out" | grep -E "Error|error" | head -3 | cut -c1-400
}
run c-val validator "$(val '')"
run v1-key-in-blocks_per_epoch validator "$(val "blocks_per_epoch = \"0x$MARK\"")"
run v2-key-in-key_gen_timeout validator "$(val "key_gen_timeout = \"0x$MARK\"")"
run c-sen sentinel "$(sen 6 '"http://localhost:5473"')"
run s1-key-in-voting_window sentinel "$(sen "\"0x$MARK\"" '"http://localhost:5473"')"
run s2-bare-hex-in-engine sentinel "$(sen 6 "\"$MARK\"")"
run s3-0x-hex-in-engine sentinel "$(sen 6 "\"0x$MARK\"")"
run s4-key-in-fee_token sentinel "$(sen 6 '"http://localhost:5473"' | sed "s/^fee_token = .*/fee_token = \"0x$MARK\"/")"
