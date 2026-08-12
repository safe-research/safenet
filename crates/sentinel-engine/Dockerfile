# ---- Builder Stage ----
# Compiles the release binary against the full workspace: `sentinel-engine`
# depends on `safenet-core`/`safe-tx` via workspace path dependencies, so the
# build needs the whole `crates/` tree.
FROM rust:1-slim AS builder
WORKDIR /usr/src/app

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

RUN cargo build --release --locked --package sentinel-engine

# ---- Runtime Stage ----
# Slim image with only the compiled binary and CA roots for the TLS endpoints
# used by the engine's checks.
FROM debian:trixie-slim AS runner
WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y --no-install-recommends \
	ca-certificates \
	&& rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/sentinel-engine ./sentinel-engine

# `ENTRYPOINT` (not `CMD`) so that a Kubernetes/`podman kube play` pod spec's
# `args:` (e.g. `--config-file=...`) appends to the binary instead of
# replacing it.
ENTRYPOINT ["./sentinel-engine"]
