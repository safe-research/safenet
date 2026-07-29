# ---- Builder Stage ----
# Compiles the release binary against the full workspace: `validator` depends
# on `safenet-core`/`safe-tx` via workspace path dependencies, so the build
# needs the whole `crates/` tree, not just `crates/validator`.
FROM rust:1-slim AS builder
WORKDIR /usr/src/app

# `pkg-config`/`libsqlite3-dev` are needed to link against the system SQLite
# (the workspace's `sqlx` dependency does not enable the `bundled` feature).
RUN apt-get update && apt-get install -y --no-install-recommends \
	libsqlite3-dev \
	pkg-config \
	&& rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

RUN cargo build --release --locked --package validator

# ---- Runtime Stage ----
# Slim image with only the compiled binary and the shared libraries it needs:
# `ca-certificates` for TLS to the RPC endpoint, `libsqlite3-0` because the
# binary links against the system SQLite rather than bundling it.
FROM debian:trixie-slim AS runner
WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y --no-install-recommends \
	ca-certificates \
	libsqlite3-0 \
	&& rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/validator ./validator

# `ENTRYPOINT` (not `CMD`) so that a Kubernetes/`podman kube play` pod spec's
# `args:` (e.g. `--config-file=...`) appends to the binary instead of
# replacing it.
ENTRYPOINT ["./validator"]
