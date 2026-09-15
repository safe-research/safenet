# Safenet Sentinel Engine

The sentinel engine is the transaction-verification service behind a Safenet sentinel. The sentinel sends every proposed Safe transaction to its configured engine, maps the returned verdict to a vote (or no vote), and handles all bond and onchain activity itself.

The reference implementation lives in [`crates/sentinel-engine`](../crates/sentinel-engine). An operator can run it as-is or replace it with another implementation of the same HTTP contract.

## Responsibilities and Trust Boundary

An engine evaluates transactions. It does not participate in the SentinelOracle protocol directly:

- It has no private signing key.
- It holds no bond.
- It makes no onchain writes.

Those are the engine's deployment invariants; being stateless is not. The reference implementation does not need a database, but a custom engine may persist threat intelligence, simulation results, or other state in one.

The engine's verdict describes the **transaction**. Approving and denying describe the **sentinel's vote**, and that vote is what the sentinel's bond is staked on:

| Engine verdict | Sentinel action |
| --- | --- |
| `secure` | Cast an approving vote. |
| `insecure` with a rule citation | Cast a denying vote carrying that citation. |
| `abstain` | Do not vote. |

An `abstain` response is successful and deliberate. It must not be interpreted as either a secure or insecure transaction.

## API Contract

[`crates/sentinel-engine/openapi.yaml`](../crates/sentinel-engine/openapi.yaml) is the authoritative interface contract. It defines the `POST /v1/security-check` request and response bodies, wire formats, and the optional `x-request-id` and `x-request-timeout` headers. Operators implementing their own engine should validate against and remain compatible with that document.

The request body's `block` field is the caller's (the sentinel's) own declared current block — the most recent block it had synced past when it submitted the check. RPC-derived checks should evaluate against this rather than resolving "latest" themselves, so the engine shares the same view of the chain the sentinel had instead of racing ahead of (or behind) it. This also lets a historical transaction be replayed against the block range it actually happened near (e.g. a `sentinel-test-vectors` vector supplying its own block) instead of however far the chain has moved on since — the caller is responsible for supplying a block _before_ the transaction being checked, since a query reaching up to the transaction's own block would see that transaction's own effects as if they were prior evidence.

Rule citations are intentionally open-ended: the Charter can gain rules without requiring the sentinel to know a closed enum of every possible citation.

The API currently specifies no authentication or rate limiting. A sentinel and its engine are expected to be co-deployed on the same host, in the same pod, or on a private network. Do not expose the reference API publicly without adding appropriate access controls at the deployment boundary.

## Configuration

Configure the reference engine by writing a TOML configuration file — see [`crates/sentinel-engine/src/config.rs`](../crates/sentinel-engine/src/config.rs) for the full schema, and copy [`sentinel-engine.sample.toml`](../crates/sentinel-engine/sentinel-engine.sample.toml) as an example to start from. The engine reads `sentinel-engine.toml` by default; pass a different path with `--config-file`.

```sh
cp crates/sentinel-engine/sentinel-engine.sample.toml sentinel-engine.toml
```

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
# poisoning check, counting back from the request's `block` field.
address_poisoning_lookback_blocks = 50000

# Optional; unset by default, which issues the whole lookback window above
# as a single eth_getLogs call. Many RPC providers cap how wide a single
# call's block range can be (Infura, for example, rejects a call spanning
# more than 10,000 blocks) — set this to that provider's own limit and the
# lookback window is split into consecutive calls no wider than it. This is
# a toBlock-fromBlock span, not a block count: if a provider instead
# documents its cap as an inclusive block count N, set this to N - 1.
# address_poisoning_max_block_range = 10000

[observability]
# Optional; defaults to "info".
log_filter = "info"

# Optional; defaults to an ephemeral port on loopback.
# metrics_address = "0.0.0.0:3556"
```

| Setting | Required | Description |
| --- | --- | --- |
| `rpc` | Yes | RPC endpoint used by checks that query chain state. |
| `bind_address` | No | HTTP listen address; defaults to `127.0.0.1:5473`. |
| `engine.blocklist` | Yes | Destinations treated as known malicious by the blocklist check. |
| `engine.address_poisoning_lookback_blocks` | Yes | Recent block range inspected for an established interaction. |
| `engine.address_poisoning_max_block_range` | No | Widest `eth_getLogs` block span the RPC allows per call; unset issues the lookback as one call. |
| `observability.log_filter` | No | `tracing` filter; defaults to `info`. |
| `observability.metrics_address` | No | Prometheus listener; defaults to an ephemeral loopback port. |

The reference engine has one RPC endpoint. Configure it for the same chain as the transactions it receives. The address-poisoning check (the only check backed by RPC-derived state) abstains on a `chainId` mismatch between a transaction and the configured RPC; other checks make no onchain calls and so have nothing to validate against the RPC's chain.

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

The provided OCI image uses the engine binary as its entrypoint. For example, create a private network shared with the sentinel, set `bind_address = "0.0.0.0:5473"` in the engine config, and run:

```sh
docker network create safenet-sentinel
docker run --name safenet-sentinel-engine \
    --network safenet-sentinel \
    --volume "$(pwd)/sentinel-engine.toml:/usr/src/app/sentinel-engine.toml:ro" \
    ghcr.io/safe-research/safenet-sentinel-engine:main \
    --config-file=sentinel-engine.toml
```

Configure the paired sentinel with the engine's base URL. The sentinel appends the versioned API path itself:

```toml
[sentinel]
engine = "http://safenet-sentinel-engine:5473"
```

Use `http://127.0.0.1:5473` instead when both processes share a network namespace or run directly on the same host.

The repository's [devnet](./devnet.md) demonstrates the container topology: `carol` and `dave` each run with a dedicated engine in the same Podman pod.

## Verdict Composition

The reference engine does not return the first check's non-`abstain` verdict. Ordering checks that way would make the engine's answer a function of checker order rather than of the transaction, and would let a `secure` from a check that only looked at _part_ of a transaction stand in for the whole thing — for example, a check that recognizes a nested `execTransaction` call has said nothing about whether the outer transaction's gas refund is reasonable.

Instead, the reference engine composes verdicts from two internal concepts that do not appear on the wire:

- **Aspect** — a part of a Safe transaction a check can vouch for: `To` (the destination), `Value` (native currency leaving the Safe), `Data` (the calldata and the effects it encodes), `Operation` (`CALL` versus `DELEGATECALL`), and `Refund` (the Safe's own gas-refund payment — `gasPrice`, `gasToken`, `safeTxGas`, `baseGas` and `refundReceiver` together, since a claim about part of the payment says nothing actionable). `chainId`, `safe` and `nonce` are deliberately not aspects: they identify which proposal is being assessed rather than describing what it does.
- **Coverage** — a set of aspects. Each check's affirming path returns the coverage it vouches for, not a bare `secure`. A deny-only check (one with no affirming path at all) has no coverage to claim.

The engine's fold works as follows: any check's denial is the engine's verdict immediately — denials are never masked by a later affirmation. Otherwise, the engine unions the coverage claimed by every affirming check. It answers `secure` only when that union contains every aspect the transaction actually requires a voucher for, and `abstain` otherwise.

Not every aspect is required for every transaction. An aspect a transaction cannot actually exercise is trivially covered and dropped from the requirement — a `value` of zero (or a `DELEGATECALL`, which takes no value argument) needs no `Value` voucher, a `gasPrice` of zero needs no `Refund` voucher (`Safe.sol` only calls `handlePayment` `if (gasPrice > 0)`), and empty `data` needs no `Data` voucher. `To` and `Operation` are always required.

**What a `To` claim does and does not assert.** `To` coverage means "no rule in scope forbids this destination." It does not mean "this destination is trustworthy." The reference engine's `BaseChecker` is the sole supplier of `To` (and `Operation`) coverage for an ordinary call, on the strength of that restriction rather than a positive statement about the destination — the destination has been checked against every Charter `to`-restriction currently enforced (a self-call confined to an allow-listed settings function; a delegatecall confined to a known migration, signing-library, `CreateCall` or MultiSend contract) and given the blocklist check its chance to deny. A destination-reputation check that makes `To` a positive statement is a possible future addition, not something the current engine does.

The consequence for an operator: **an engine answering `secure` is asserting that it examined every aspect the transaction has** — not merely that some check liked part of it and nothing else objected. An `abstain` from the reference engine most often means some aspect (frequently `Refund`, on a relayed transaction none of the affirming checks look at) had no voucher, not that a check actively distrusted the transaction. This is also why implementing a custom engine's own composition rule matters as much as implementing its individual checks: a custom engine that returns the first non-`abstain` verdict from its own checks reintroduces the exact ordering-dependence problem described above.

## Implementing a Custom Engine

A custom engine may use different checks, external services, simulations, or persistent storage. It must preserve the [OpenAPI contract](../crates/sentinel-engine/openapi.yaml) and the trust boundary above: it assesses transactions but does not hold the sentinel key, put up bonds, or write to the SentinelOracle. This keeps transaction-verification compromise separate from custody and onchain participation.
