#!/usr/bin/env bash
# rv-xc run 3: dependency reachability for F-XC-011 (advisories) and F-XC-007 (gate and feature
# width), re-run at the audit commit against the run-3 `cargo audit` log
# (rust-audit/state/run3/logs/05-audit.log). Run from the checkout to inspect.
set -u
echo "# head: $(git rev-parse HEAD)"
echo "# cargo: $(cargo --version)"
for spec in crossbeam-epoch@0.9.18 h2@0.4.14 quinn-proto@0.11.14 ruint@1.18.0 rustls@0.23.40 \
            atomic-polyfill@1.0.3 derivative@2.2.0 paste@1.0.15 proc-macro-error2@2.0.1 \
            anyhow@1.0.102 event-listener@5.4.1 lru@0.16.4 bitcoin-io@0.1.100 \
            bitcoin_hashes@0.14.100 chacha20@0.10.0 spin@0.9.8; do
  echo "=================== cargo tree --offline --locked -p safenet-core -p validator -p sentinel -i $spec -e normal --depth 4"
  cargo tree --offline --locked -p safenet-core -p validator -p sentinel -i "$spec" -e normal --depth 4 2>&1 | head -40
done
echo "=================== reqwest features, per binary (is http2 on?)"
for p in validator sentinel; do
  echo "--- -p $p -e features -i reqwest --depth 1"
  cargo tree --offline --locked -p "$p" -e features -i reqwest --depth 1 2>&1
done
echo "=================== hyper features reaching the binaries (http2 comes only through hyper-util server-auto)"
cargo tree --offline --locked -p validator -p sentinel -e features -i hyper --depth 2 2>&1 | head -40
echo "=================== metrics-exporter-prometheus: the listener is HTTP/1 only"
SRC=$(ls -d ~/.cargo/registry/src/*/metrics-exporter-prometheus-0.18.3 | head -1)
grep -n 'http1::Builder\|serve_connection\|auto::\|http2' "$SRC/src/exporter/http_listener.rs"
echo "=================== c-kzg / blst (alloy full -> kzg) still compiled into the binaries?"
cargo tree --offline --locked -p validator -p sentinel -i c-kzg -e normal --depth 3 2>&1 | head -20
echo "=================== metrics-exporter-prometheus features enabled (push-gateway default?)"
cargo tree --offline --locked -p safenet-core -e features -i metrics-exporter-prometheus --depth 1 2>&1 | head -20
echo "=================== workspace-wide, any edge kind, any target, for the lockfile-only crates"
for c in quinn-proto derivative anyhow bitcoin-io bitcoin_hashes chacha20; do
  printf '%s: ' "$c"; cargo tree --offline --locked --workspace -i "$c" -e all --target all 2>&1 | head -1
done
echo "=================== rayon / crossbeam-deque (removed since fe9e84c?)"
for c in rayon rayon-core crossbeam-deque; do
  printf '%s: ' "$c"; grep -c "^name = \"$c\"" Cargo.lock
done
