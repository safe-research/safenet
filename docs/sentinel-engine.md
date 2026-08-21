# Safenet Sentinel Engine

The sentinel engine is the transaction-verification service behind a Safenet sentinel. The
sentinel sends every proposed Safe transaction to its configured engine, maps the returned verdict
to a vote (or no vote), and handles all bond and onchain activity itself.

The reference implementation lives in [`crates/sentinel-engine`](../crates/sentinel-engine). An
operator can run it as-is or replace it with another implementation of the same HTTP contract.

## Responsibilities and Trust Boundary

An engine evaluates transactions. It does not participate in the SentinelOracle protocol directly:

- It has no private signing key.
- It holds no bond.
- It makes no onchain writes.

Those are the engine's deployment invariants; being stateless is not. The reference implementation
does not need a database, but a custom engine may persist threat intelligence, simulation results,
or other state in one.

The engine's verdict describes the **transaction**. Approving and denying describe the
**sentinel's vote**, and that vote is what the sentinel's bond is staked on:

| Engine verdict                  | Sentinel action                             |
| ------------------------------- | ------------------------------------------- |
| `secure`                        | Cast an approving vote.                     |
| `insecure` with a rule citation | Cast a denying vote carrying that citation. |
| `abstain`                       | Do not vote.                                |

An `abstain` response is successful and deliberate. It must not be interpreted as either a secure
or insecure transaction.

## API Contract

[`crates/sentinel-engine/openapi.yaml`](../crates/sentinel-engine/openapi.yaml) is the authoritative
interface contract. It defines the `POST /v1/security-check` request and response bodies, wire
formats, and the optional `x-request-id` and `x-request-timeout` headers. Operators implementing
their own engine should validate against and remain compatible with that document.

Rule citations are intentionally open-ended: the Charter can gain rules without requiring the
sentinel to know a closed enum of every possible citation.

The API currently specifies no authentication or rate limiting. A sentinel and its engine are
expected to be co-deployed on the same host, in the same pod, or on a private network. Do not expose
the reference API publicly without adding appropriate access controls at the deployment boundary.

## Configuration

The reference engine reads TOML from `sentinel-engine.toml` by default. Pass a different path with
`--config-file`.

```toml
# RPC used by checks that read onchain state.
rpc = "https://rpc.gnosischain.com"

# Optional; defaults to 127.0.0.1:5473. Bind to 0.0.0.0 when another
# container must reach the engine over a container network.
bind_address = "127.0.0.1:5473"

[engine]
# Required, but may be empty.
blocklist = []

# Number of recent blocks searched for prior interactions by the address-
# poisoning check.
address_poisoning_lookback_blocks = 50000

[observability]
# Optional; defaults to "info".
log_filter = "info"

# Optional; defaults to an ephemeral port on loopback.
# metrics_address = "0.0.0.0:3556"
```

| Setting                                    | Required | Description                                                     |
| ------------------------------------------ | -------- | --------------------------------------------------------------- |
| `rpc`                                      | Yes      | RPC endpoint used by checks that query chain state.             |
| `bind_address`                             | No       | HTTP listen address; defaults to `127.0.0.1:5473`.              |
| `engine.blocklist`                         | Yes      | Destinations treated as known malicious by the blocklist check. |
| `engine.address_poisoning_lookback_blocks` | Yes      | Recent block range inspected for an established interaction.    |
| `observability.log_filter`                 | No       | `tracing` filter; defaults to `info`.                           |
| `observability.metrics_address`            | No       | Prometheus listener; defaults to an ephemeral loopback port.    |

The reference engine has one RPC endpoint. Configure it for the same chain as the transactions it
receives; it does not currently reject a request whose `chainId` differs from the RPC's chain.

## Running the Reference Engine

From a repository checkout:

```sh
cargo run --package sentinel-engine -- --config-file sentinel-engine.toml
```

Or build the release binary first:

```sh
cargo build --release --package sentinel-engine
./target/release/sentinel-engine --config-file sentinel-engine.toml
```

The provided OCI image uses the engine binary as its entrypoint. For example, create a private
network shared with the sentinel, set `bind_address = "0.0.0.0:5473"` in the engine config, and
run:

```sh
docker network create safenet-sentinel
docker run --name safenet-sentinel-engine \
    --network safenet-sentinel \
    --volume "$(pwd)/sentinel-engine.toml:/usr/src/app/sentinel-engine.toml:ro" \
    ghcr.io/safe-research/safenet-sentinel-engine:main \
    --config-file=sentinel-engine.toml
```

Configure the paired sentinel with the engine's base URL. The sentinel appends the versioned API
path itself:

```toml
[sentinel]
engine = "http://safenet-sentinel-engine:5473"
```

Use `http://127.0.0.1:5473` instead when both processes share a network namespace or run directly
on the same host.

The repository's [devnet](./devnet.md) demonstrates the container topology: `carol` and `dave`
each run with a dedicated engine in the same Podman pod.

## Implementing a Custom Engine

A custom engine may use different checks, external services, simulations, or persistent storage.
It must preserve the [OpenAPI contract](../crates/sentinel-engine/openapi.yaml) and the trust
boundary above: it assesses transactions but does not hold the sentinel key, put up bonds, or write
to the SentinelOracle. This keeps transaction-verification compromise separate from custody and
onchain participation.
