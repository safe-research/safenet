# Plan: Sentinel Engine

Component: `crates/sentinel` (Rust), new `crates/sentinel-engine` (Rust), `crates/safe-tx` (Rust), plus `scripts/` and `docs/`.

---

## Overview

`crates/sentinel` currently does two unrelated jobs in one process. It is a **protocol participant**: it watches `Consensus`/`SentinelOracle`, drives the commit-reveal FSM (`state.rs`, `service.rs`), signs and submits `commit`/`reveal`/`finalize`/`claim` transactions, and manages its bond. It is also a **transaction verifier**: `static_checker.rs` runs the Charter's deterministic base guarantees plus a destination blocklist, `cow.rs` (1297 lines) decodes CoW Protocol batches and calls CoW's public order API, `address_poisoning.rs` issues `eth_getLogs` lookups, and `dynamic_checker.rs` POSTs to an optional operator-configured endpoint for whatever isn't implemented locally.

These two jobs have nothing in common. The protocol side is fixed by the contracts and must be identical across every sentinel operator; the verification side is exactly where operators are meant to differentiate, iterate quickly, and take on liability. Today they share a process, a crate, a config file, a release cadence, and a deployment. `dynamic_checker.rs`'s own module docs already name the seam ("`RemoteChecker`'s `Checker` impl is already the seam to split _trigger this endpoint, parse the response_ along if that ever needs to move out on its own").

This epic promotes that seam into a real service boundary:

- **`crates/sentinel-engine`** — a new HTTP service. It accepts a proposed Safe transaction and answers whether it is **secure**. It holds no signing key, no bond, and no onchain write access; the version this epic delivers also keeps no state between requests and needs no database, though that is a property of the checks it happens to run rather than of the contract (see Architecture Decision). Every checker in the repo today (`static_checker.rs`, `cow.rs`, `address_poisoning.rs`) moves here, unchanged. It also holds the hand-authored **`openapi.yaml`** that is the interface's source of truth.
- **`crates/sentinel`** — keeps only the protocol side. Its checker chain collapses to a single HTTP call, and it ends the epic with zero transaction-verification logic in it.

Sequencing is driven by one hard constraint: `crates/sentinel` is a binary crate with private modules, so **a checker cannot exist in both crates at once**. Every move therefore lands as a paired "add to engine / drop from sentinel" change. The phases are ordered so that the only checker temporarily out of service (Phases 5-6) is `address_poisoning.rs`, which by its own module docs "does not yet deny anything" — so no verdict observable onchain changes while it is dormant. `cow.rs` and `static_checker.rs`, which do deny, only move _after_ the sentinel is already talking to the engine.

**Prerequisite: `epics/2026_07_24_nonblocking_effects.md` must land first.** Phase 4 makes _every_ proposal go through `Effect::DynamicCheck` (today a local blocklist denial short-circuits inline), and Phase 6 turns that effect into a network round trip. Under today's fully-blocking effects (`TransitionBatch::apply` awaits `perform_effect` under a single `Mutex`), that would stall the sentinel's entire transition loop on an HTTP request once per proposal — strictly worse than today. That epic also delivers the `WaitingForDynamicCheck` state this epic's FSM needs to track a proposal whose check is outstanding.

---

## Architecture Decision

**The engine is synchronous request/response, and untrusted.** One `POST` in, one verdict out: no queue, no callback, no signer, no bond, and no onchain access other than read-only RPC for its own checks. Those are the invariants, and they keep the trust story simple: a compromised or buggy engine can only ever make its sentinel vote wrongly (or not at all) — it can never spend the sentinel's bond, sign anything, or corrupt the sentinel's FSM. It also means an operator can restart, rewrite, or replace their engine without touching the process that holds their key and their in-flight requests.

**This engine is stateless, but "no database" is a property of these checks, not a rule of the interface.** Every check that exists today is a pure function of the transaction plus live lookups (`eth_getLogs`, the CoW order API), so the version this epic delivers holds nothing between requests: no `sqlx`, no migrations, no volume in the devnet pod. It is worth being explicit that this is not an architectural constraint being imposed on future engines. A threat-intel cache, memoized simulation results, a local mirror of an allow-list feed, or per-caller rate limiting all plausibly want a store, and none of them change the contract, because the API carries no session and no cross-request identity — a request is answerable on its own. What an engine must not have is a key, a bond, or write access to a chain.

**"Secure"/"insecure" describes the transaction; "approve"/"deny" describes the vote.** The engine answers a question about the transaction, in the Charter's terms. The sentinel translates that answer into the vote `SentinelOracle` understands (`commitApprove`/`commitDeny`, `Vote.APPROVED`/`Vote.DENIED`). Keeping the two vocabularies distinct is deliberate even though they map 1:1 today: the engine has no opinion about voting, bonding, or whether its caller is a sentinel at all, and the sentinel's vote is a commitment backed by its bond, which is a strictly bigger claim than "my engine said this looks insecure".

**One engine per sentinel, not one shared engine.** Independent verification is the whole point of having multiple sentinels; a shared engine would make them vote identically by construction and collapse the network's fault tolerance for the verification half of the protocol. The devnet and integration scripts therefore start one engine per sentinel, on its own port. Nothing in the code enforces this — it is a deployment property — but the tooling models it correctly so nobody learns the wrong shape from the devnet.

**`openapi.yaml` is the contract; each side has its own request/response envelope, pinned to it by golden-JSON tests.** The spec lives in `crates/sentinel-engine/`, next to the reference implementation of it. The engine's `CheckRequest`/`Verdict` live in its `api.rs`; the sentinel's live in its `engine.rs`, exactly as `dynamic_checker.rs`'s `Request`/`Response` do today. The transaction inside them is the one type both sides share, from `safe_tx::wire` (below) — but the two services are still not wired together by a shared API crate: the spec plus a golden-JSON fixture on each side is what keeps them in agreement, and that is also the only mechanism available to a third-party engine written in another language. A dedicated shared types crate was considered and rejected (see Alternatives).

**The verdict becomes a three-state enum.** Today the same concept exists three times over with different spellings: `CheckOutcome::{Approved, Denied(RuleId), Unknown}` internally, `Decision { approve: bool, reason: Cow<str> }` out of `StaticChecker`, and `{"approve": bool, "rule": Option<String>}` on `RemoteChecker`'s wire — where `{"approve": false, "rule": null}` means "no verdict", conflating denial with failure. All three become one shape, spelled the same on both sides of the wire:

```rust
#[serde(tag = "verdict", rename_all = "lowercase")]
pub enum Verdict {
    Secure,
    Insecure { rule: RuleId },
    Inconclusive,
}
```

`Inconclusive` is a **successful** `200` response, not an error: "I could not reach the CoW API" is a legitimate, honest answer that the sentinel must not read as either approval or denial. It is also what the sentinel maps a transport failure or a non-`200` to, so the FSM has exactly one "no trustworthy verdict, drop the request unanswered" path rather than two. `Insecure` carries the `RuleId` as a required field, so the "denied without a recognized rule code" case `dynamic_checker.rs` currently has to detect at runtime becomes unrepresentable on the wire.

**The wire types are hand-written DTOs, not the `sol!`-generated structs.** `safe_tx::types::SafeTransaction` derives serde off `alloy::sol!`, which fixes an encoding we would not choose to publish as a contract: addresses come out lower-case, and `sol!` appends a hidden `__Invalid` variant annotated `#[serde(other)]` to `Operation`, so `"operation": "SELFDESTRUCT"` deserializes _successfully_. Those derives also cannot simply be changed — `validator`'s `Packet` serializes `SafeTransaction` into signed/persisted state, which is a different encoding with a different compatibility story than an HTTP API. So the JSON form gets its own type: `safe-tx` gains a `wire.rs` holding a hand-written `SafeTransaction`/`Operation` pair with conversions to and from the `sol!` types, and both the engine and the sentinel use it. That buys three things the derived encoding cannot: checksummed addresses, an `Operation` that is exactly `{CALL, DELEGATECALL}` with no escape hatch, and `deny_unknown_fields` on the request, where an ignored member could mean checking a different transaction than the one proposed. The 256-bit quantities and `bytes` keep `alloy`'s `U256`/`Bytes` and their encodings (minimal lower-case hex) — the Ethereum JSON-RPC conventions, and what we would have picked anyway.

`wire.rs` lives in `safe-tx` rather than being duplicated per side because it is the JSON encoding of a Safe transaction, which is squarely that crate's subject (it already owns the `sol!` `SafeTransaction`, `Operation`, and `RuleId`), and because the conversion to the type the checks actually take has to exist somewhere. The API's own envelope — `CheckRequest` and `Verdict`, about 25 lines each — stays duplicated on each side, as below.

**Addresses are emitted checksummed and accepted in any case.** EIP-55 mixed case is the form every Safe UI, block explorer, and audit trail shows, so it is what the engine's responses, its logs, and the JSON a human pastes into `curl` should agree on. The decoder is deliberately more lenient than the encoder: `Address::from_str` (case-insensitive), not `Address::parse_checksummed`. Rejecting lower-case input would be hostile to hand-written clients and to third-party engines whose address type does not checksum on the way out, and a checksum is a typo guard rather than a security property here — a wrong-but-well-checksummed address is exactly as dangerous.

**The spec is hand-authored, not generated.** It is a contract third-party engines implement, not a byproduct of our reference server: it carries the prose that makes `inconclusive` unambiguous, the examples both crates' golden tests assert against, and the deliberate open-endedness of `rule`. Generating it from the handlers (`utoipa`) was rejected (see Alternatives). What YAML cannot enforce on its own is that our Rust actually emits what it describes — golden-JSON fixtures asserted against the real types do that, and they also catch the case where an `alloy` upgrade changes `U256`/`Bytes` encoding underneath the DTOs.

**Liveness and metrics stay on the shared observability listener.** `safenet_core::observability` already serves `/health` and `/metrics` on its own `metrics_address`; the engine reuses it verbatim rather than adding a second health endpoint on the API port. `openapi.yaml` therefore describes exactly one path, and says so explicitly — the API port serves the API, nothing else.

**`RuleId` gains serde in `safe-tx`, encoded as its Charter citation.** `RuleId::code()`/`from_code()` already exist for exactly this purpose (`from_code`'s doc comment names "validating a code an external check service cites in its response" as its motivation). Adding `Serialize`/`Deserialize` on top of them keeps the Charter citation the single canonical wire form on both sides, and moves the "unrecognized rule code" failure from a runtime `tracing::error!` into a deserialization error at the HTTP boundary.

**The set of rule citations is open-ended, and the spec says so.** The Charter is a living document: rules are added, and `RuleId` is grown "incrementally, in the same change that implements the check giving it meaning". So `openapi.yaml` describes `rule` by its shape (`^R-[0-9]+\.[0-9]+$`) with the currently-implemented citations as `examples`, not as a closed `enum` that would be wrong the day the Charter gains a rule — an `enum` there would also read as a claim about the Charter, which this document is not entitled to make. A closed set does still exist at any given moment inside a given build: `safe-tx`'s `RuleId` recognizes exactly the citations whose checks it implements, and rejects the rest on deserialization. The consequence is that an engine citing a rule newer than its sentinel's `safe-tx` gets read as `Inconclusive` — see Open Questions for why that is the right failure and how to avoid it.

**The sentinel's engine URL is a required base URL, and requests carry a timeout.** `remote_check_url` is optional today, and unconfigured means "approve everything the local checks did not deny" — a sensible default when the remote check was an optional extra, and a dangerous one once it is the _only_ verification. It becomes mandatory: a sentinel with no engine is misconfigured, and should fail loudly at startup rather than vote on transactions nobody checked. The value is a base URL (`http://localhost:8080`), with the client appending the spec'd `v1/check` path, since the path belongs to the contract rather than to the deployment. It also gains an explicit request timeout (`reqwest`'s default is _none_, so today a hung endpoint holds the effect open until the request's block deadline sweeps it).

### Alternatives Considered

- **A shared `sentinel-engine-api` crate holding the wire types, depended on by both sides.** It would make client/server agreement structural rather than test-enforced, and give a Rust third-party engine something light to depend on. Rejected as over-engineering: it is a `Cargo.toml` and a workspace entry to hold `CheckRequest` and `Verdict` — the part not already shared via `safe_tx::wire` — which is about 25 lines of `struct`/`enum` per side, to solve a drift problem that a golden-JSON fixture on each side already solves, and solves for engines written in any language, which a Rust crate does not. It would also be the third crate in the workspace whose only job is to hold types. The status quo (`dynamic_checker.rs` declaring its own `Request`/`Response`) is already this shape and has caused no trouble. Note this is a weaker rejection than it was before `safe_tx::wire`: had the whole 12-field transaction stayed duplicated, a shared crate would have been the better call.
- **Keep the deterministic `StaticChecker` in the sentinel, move only the dynamic checks (CoW, address poisoning) to the engine.** This is the smaller change and preserves the appealing property that a blocklist hit is decided locally, synchronously, with no network dependency. Rejected: it does not achieve the split. The sentinel would still contain Charter rule logic (`safe_tx::checks`, the blocklist, the unlimited-approval decoder), still need `blocklist` config, and still have to be redeployed to change a verification rule. It would also leave verification split across two components with no principle for deciding where a _new_ check goes. The cost of moving it — an engine outage means the sentinel votes on nothing at all, rather than still landing blocklist denials — is the correct fail-closed behavior anyway: no vote means no bond at risk.
- **Generate `openapi.yaml` from the Rust handlers with `utoipa`.** Attractive for drift, and now technically available since the wire types are hand-written DTOs that could derive `ToSchema`. Rejected anyway: it inverts the intended relationship — the YAML is a contract third-party engines implement, not a byproduct of our reference server — and most of the file's value is precisely what a derive cannot express. That `inconclusive` is a success and must not be read as approval, that `rule` is an open set the Charter grows, that addresses are emitted checksummed but accepted in any case, that liveness deliberately lives on another port: all of it would become `#[schema(description = "...")]` attributes stuffed into the DTOs, i.e. the same prose, less readable, reviewed in diffs of Rust rather than of the contract. And the drift it protects against is the cheap half; golden-JSON tests already cover the expensive half (what the types actually emit).
- **Have the engine push verdicts back (callback URL, webhook, or a streaming/subscription protocol) instead of answering inline.** Rejected: the sentinel already has a general mechanism for "an outstanding operation whose answer may never arrive" (the background effect plus its per-request deadline sweep), so an inline request/response is all it needs. A callback would require the sentinel to expose a listening socket — which the project deliberately avoids (`docs/overview.md`: onchain communication "reduces the operational complexity of running a validator, as you do not need to expose a service to the scary internet") — and would add request correlation, retry, and authentication problems the synchronous shape simply does not have.
- **Give the engine a per-chain RPC map so `transaction.chainId` actually selects the endpoint its onchain lookups use.** Out of scope, deliberately. Today `AddressPoisoningChecker` is handed the same provider the event watcher uses, i.e. the Safenet coordination chain, which is not necessarily the chain the Safe transaction targets (`run_sentinel_integration_test.sh` proposes with `TX_CHAIN_ID=1` against a chain-31337 Anvil). That is a real, pre-existing bug, but it is a bug about which chain a check reads from, not about where verification lives — folding it in would mix a behavior fix into a refactor. It gets materially easier to fix afterwards, since the engine will own both the RPC configuration and the verification logic. See Open Questions.
- **Serve the engine on `hyper` directly instead of adding `axum`.** `hyper` 1.x and `tower`/`tower-http` are already in `Cargo.lock` transitively, so raw `hyper` adds no new top-level dependency for what is one `POST` route. Rejected: the hand-rolled version has to get body-size limits, method/path routing, content-type negotiation, and error mapping right by hand — all of which `axum` provides, on the same `hyper` version already in the tree.

---

## Tech Specs

### `openapi.yaml`

Lives at `crates/sentinel-engine/openapi.yaml`. OpenAPI 3.1.0, one path. Excerpt (the phase delivers the complete file):

```yaml
openapi: 3.1.0
info:
  title: Safenet Sentinel Engine API
  version: 0.1.0
  summary: Transaction verification for Safenet sentinels.
  description: >-
    A sentinel watching the Consensus contract POSTs each proposed Safe
    transaction here and votes according to the verdict it gets back. An
    engine holds no signing key, no bond, and no write access to any chain:
    it answers questions about transactions and nothing else. Every request
    is answerable on its own -- the API carries no session and no
    cross-request state -- though an engine is free to keep internal state
    (caches, threat-intel mirrors) if its checks need it. The reference
    implementation does not.

    Liveness and metrics are not part of this API. The reference implementation
    serves `/health` and `/metrics` on a separate observability listener
    (`metrics_address`), not on this port.
  license:
    name: Apache-2.0
    identifier: Apache-2.0
servers:
  - url: http://localhost:8080
    description: Default local bind address of the reference implementation.
paths:
  /v1/check:
    post:
      operationId: checkTransaction
      summary: Decide whether a proposed Safe transaction is secure.
      parameters:
        - name: x-request-id
          in: header
          required: false
          description: >-
            Opaque correlation id, echoed back on the response and included in
            the engine's logs. The reference sentinel sends the Safenet request
            id of the proposal being checked.
          schema:
            type: string
            maxLength: 128
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: "#/components/schemas/CheckRequest"
      responses:
        "200":
          description: >-
            A verdict. Note that `inconclusive` is a successful response: the
            engine is reporting that it has no trustworthy answer, which a
            caller must not read as either secure or insecure.
          content:
            application/json:
              schema:
                $ref: "#/components/schemas/Verdict"
        "400":
          description: The request body is not a well-formed CheckRequest.
        "415":
          description: The request body is not `application/json`.
        "500":
          description: The engine failed unexpectedly.
        "503":
          description: The engine is starting up or shutting down.
components:
  schemas:
    CheckRequest:
      type: object
      additionalProperties: false
      required: [transaction]
      properties:
        transaction:
          $ref: "#/components/schemas/SafeTransaction"
    SafeTransaction:
      description: >-
        The 12-field SafeTransaction tuple carried by Consensus's
        `OracleTransactionProposed` event.
      type: object
      additionalProperties: false
      required:
        [
          chainId,
          safe,
          to,
          value,
          data,
          operation,
          safeTxGas,
          baseGas,
          gasPrice,
          gasToken,
          refundReceiver,
          nonce,
        ]
      properties:
        chainId: { $ref: "#/components/schemas/Quantity" }
        safe: { $ref: "#/components/schemas/Address" }
        to: { $ref: "#/components/schemas/Address" }
        value: { $ref: "#/components/schemas/Quantity" }
        data: { $ref: "#/components/schemas/Bytes" }
        operation: { $ref: "#/components/schemas/Operation" }
        safeTxGas: { $ref: "#/components/schemas/Quantity" }
        baseGas: { $ref: "#/components/schemas/Quantity" }
        gasPrice: { $ref: "#/components/schemas/Quantity" }
        gasToken: { $ref: "#/components/schemas/Address" }
        refundReceiver: { $ref: "#/components/schemas/Address" }
        nonce: { $ref: "#/components/schemas/Quantity" }
    Operation:
      description: >-
        The Safe call type. Exactly these two values; anything else is a
        malformed request, not an unknown-but-tolerated one.
      type: string
      enum: [CALL, DELEGATECALL]
    Verdict:
      description: >-
        The engine's answer about the transaction. A sentinel maps `secure` to
        an approving vote and `insecure` to a denying one, but that mapping is
        the sentinel's business: the engine describes the transaction, not the
        vote.
      oneOf:
        - $ref: "#/components/schemas/Secure"
        - $ref: "#/components/schemas/Insecure"
        - $ref: "#/components/schemas/Inconclusive"
      discriminator:
        propertyName: verdict
        mapping:
          secure: "#/components/schemas/Secure"
          insecure: "#/components/schemas/Insecure"
          inconclusive: "#/components/schemas/Inconclusive"
    # Verdicts, unlike requests, are open objects: a caller ignores members it
    # does not know, so an engine may return additional detail and this
    # interface can grow additively.
    Secure:
      description: >-
        The engine considers the transaction secure. A claim about the checks
        this engine runs, not a proof of safety.
      type: object
      required: [verdict]
      properties:
        verdict: { const: secure }
    Insecure:
      description: >-
        A check found the transaction to be in violation of the cited Charter
        rule.
      type: object
      required: [verdict, rule]
      properties:
        verdict: { const: insecure }
        rule: { $ref: "#/components/schemas/RuleId" }
    Inconclusive:
      description: >-
        The engine has no trustworthy answer -- a dependency it needs was
        unreachable, or no check it runs has an opinion about this
        transaction. Not an error, and not a weak `secure`.
      type: object
      required: [verdict]
      properties:
        verdict: { const: inconclusive }
    RuleId:
      type: string
      description: >-
        A Safenet Arbitration Charter rule citation. **Not a closed set**: the
        Charter is a living document, and this list grows with it. The
        `examples` below are the citations the reference implementation
        currently has checks for -- see `safe_tx::rule::RuleId` -- and are
        neither the whole Charter nor a stable set. A caller that does not
        recognize a citation has no basis to act on the verdict and should
        treat the response as unusable; the reference sentinel treats it as
        `inconclusive` and declines to vote.
      pattern: "^R-[0-9]+\\.[0-9]+$"
      examples: ["R-4.1", "R-4.2", "R-4.3", "R-4.4", "R-4.5", "R-4.6"]
    Address:
      type: string
      description: >-
        A `0x`-prefixed 20-byte address. Emitted with an EIP-55 mixed-case
        checksum; accepted in any case, with the checksum not verified (it is
        a typo guard, and a well-checksummed wrong address is just as wrong).
      pattern: "^0x[0-9a-fA-F]{40}$"
      examples: ["0x5aFE3855358E112B5647B952709E6165e1c1eEEe"]
    Quantity:
      type: string
      description: >-
        A 256-bit unsigned integer as minimal (no leading zeros), lower-case,
        `0x`-prefixed hex -- what `alloy`'s U256 serializes to. Zero is "0x0".
      pattern: "^0x([1-9a-f][0-9a-f]*|0)$"
    Bytes:
      type: string
      description: Lower-case, `0x`-prefixed byte string; empty is "0x".
      pattern: "^0x([0-9a-f]{2})*$"
```

`CheckRequest` is a wrapper object rather than a bare transaction, so the interface has somewhere to grow (per-request context, hints) without a breaking change.

**Requests are strict, responses are open.** `CheckRequest` and the transaction inside it are `additionalProperties: false` / `deny_unknown_fields` — which is only possible because `safe_tx::wire` gives the transaction a hand-written type. A rejected typo (`refundReciever`) beats a silently ignored one when the ignored member is a destination address, and a request the engine cannot fully account for is not one it should answer. The three `Verdict` variants are the opposite: unknown members are ignored, so an engine can return extra detail (an explanation, a confidence, a link to its own reasoning) and this interface can grow additively without every sentinel on the network having to upgrade first. The asymmetry is the risk asymmetry — ignoring part of a request can mean checking a different transaction than the one proposed; ignoring part of a response only means missing colour. What stays closed on the response side is the `verdict` tag itself: an unrecognized tag is a parse failure, not a fourth outcome to guess at.

The spec's `example`s are the same JSON literals the golden tests assert against, with a comment on each side pointing at the file.

### `crates/sentinel-engine`

New binary crate, `argh`-parsed `--config-file`/`--version` options matching `crates/sentinel/src/main.rs`. Dependencies: `alloy`, `argh`, `axum`, `reqwest`, `safe-tx`, `safenet-core` (observability only), `serde`, `thiserror`, `tokio`, `toml`, `tracing`, `url`. No `Signer`: the engine has no key, and that one is an invariant. No `sqlx` either, because none of the checks it runs need to remember anything — a later engine that does may well add one.

`axum` is a new workspace dependency; it runs on the `hyper` 1.x already in `Cargo.lock`. Pin to the current major at implementation time (`0.8` as of writing).

Config (`config.rs`), following `crates/sentinel/src/config.rs`'s shape. Every table is a leaf here (no `#[serde(flatten)]`, since there is no driver config), so all of them keep `deny_unknown_fields`:

```toml
rpc = "https://..."          # read-only, for the address-poisoning check
bind_address = "0.0.0.0:8080"

[engine]
blocklist = []                              # arrives in Phase 8
address_poisoning_lookback_blocks = 50000   # arrives in Phase 5

[observability]
log_filter = "info"
metrics_address = "127.0.0.1:9090"
```

`bind_address` is a `std::net::SocketAddr` with no default: a verification service silently binding to loopback (or to `0.0.0.0`) is the kind of thing that should be spelled out per deployment.

`api.rs` holds the `axum` router, the single handler, and the `CheckRequest`/`Verdict`/`ApiError` types (the transaction itself comes from `safe_tx::wire`). Behavior:

- `POST /v1/check` -> convert the request's transaction into `safe_tx::types::SafeTransaction`, run the checker chain, `200` with a `Verdict`.
- Body-size limit left at `axum`'s `DefaultBodyLimit` (2 MiB), which is far above the largest plausible MultiSend calldata blob.
- Malformed body (including an unknown member, or an `operation` outside `{CALL, DELEGATECALL}`) -> `400`; wrong content type -> `415`; both with an `ApiError`. A checker panic or unexpected internal failure -> `500`. A _dependency_ failure (unreachable RPC, unreachable CoW API) is not an error: it is `200 Inconclusive`.
- `x-request-id`, when present, is recorded as a `tracing` span field and echoed on the response, so an engine log line can be correlated with the sentinel's `%request_id`.
- Router tests use `tower::ServiceExt::oneshot` against the router directly, so they need no socket and no port.

`checker.rs` holds the `Checker` trait, retyped onto the engine's `Verdict`, and the chain runner moved out of `crates/sentinel/src/effect.rs` (run in order, stop at the first non-`Inconclusive` answer, `Inconclusive` if every checker abstains). It is introduced in Phase 5, together with its first implementor. Final chain order, preserving today's precedence exactly:

1. `StaticChecker` (today: run inline in the transition, ahead of the chain)
2. `CowChecker`
3. `AddressPoisoningChecker`

`Dockerfile` + `Dockerfile.dockerignore` modeled on `crates/sentinel/Dockerfile`, minus the SQLite build/runtime dependencies (`pkg-config`, `libsqlite3-dev`, `libsqlite3-0`) which the engine does not need; `ca-certificates` is still required for TLS to the RPC endpoint and the CoW API. A matrix entry is added to `.github/workflows/docker.yml`. `ci.yml` needs no change: it already runs `cargo clippy --workspace` and `cargo test --workspace`, which pick up `crates/*` automatically.

### Wire-format tests

The transaction's encoding is pinned once, in `safe-tx` next to the type that defines it; each side pins its own envelope and the behavior it owns at its own boundary.

In `safe_tx::wire` (Phase 1):

- A fully-populated `SafeTransaction` serializes to exactly the spec's documented JSON, and deserializes back — pinning the camelCase members, hex `Quantity` encoding (including `"0x0"` for zero and `"0x"` for empty `data`), `"CALL"`/`"DELEGATECALL"`, and the checksummed address form, character for character.
- The same address deserializes from its lower-case, upper-case, and checksummed spellings, and all three re-serialize to the checksummed one.
- `deny_unknown_fields` rejects an unknown member; an `operation` of `"SELFDESTRUCT"` fails to deserialize. **This is the case worth a test even though it looks trivial**: it is exactly what the `sol!`-generated type gets wrong (`#[serde(other)] __Invalid`), and the whole reason a hand-written DTO exists. A regression here is silent.
- Round-tripping through `safe_tx::types::SafeTransaction` and back is the identity, for a fully-populated transaction and for `Default`.

In both `crates/sentinel-engine` (what it accepts and emits) and `crates/sentinel` (what it sends and parses):

- `CheckRequest` serializes to / deserializes from the documented JSON, and rejects an unknown top-level member.
- Each of the three `Verdict` variants round-trips to and from its documented JSON, with the `"secure"`/`"insecure"`/`"inconclusive"` tags spelled out as literals. An unknown member on a verdict deserializes fine (the additive-growth path); an unknown `verdict` tag does not.
- An unrecognized `rule` string (`"R-9.9"`, well-formed but not implemented) fails to deserialize — and on the sentinel side, a router-level test that this surfaces as `Inconclusive`, not as a panic or an approval.
- Engine-side, at the HTTP boundary: a bad `operation`, an unknown member, and a non-JSON content type each get the status the spec promises.

### `crates/sentinel` after the split

`src/` loses `checker.rs`, `static_checker.rs`, `cow.rs`, `address_poisoning.rs` and `dynamic_checker.rs`; it gains `engine.rs`, holding the client, its `CheckRequest`/`Verdict` types, and the `Verdict` the FSM resumes on:

```rust
/// Asks the operator's configured `sentinel-engine` whether a proposed
/// transaction is secure.
pub struct EngineClient {
    endpoint: Url,   // the configured base URL with `v1/check` appended
    client: reqwest::Client,
}
```

Any transport failure, timeout, or non-`200` resolves to `Verdict::Inconclusive` — the same "no trustworthy verdict" path a genuine `Inconclusive` takes, logged at `error` with the cause. `effect::Handler` holds one `EngineClient` instead of a `Vec<Box<dyn Checker>>`, and `Effect::DynamicCheck`'s resume carries a `Verdict`.

Config: `[sentinel]` loses `blocklist`, `remote_check_url`, and `address_poisoning_lookback_blocks`, and gains a nested engine table:

```toml
[sentinel]
fee_token = "0x..."
voting_window = 100

[sentinel.engine]
url = "http://localhost:8080"
timeout = 10000                  # milliseconds, matching `[index] block_time`
```

`url` is mandatory. The client appends the spec'd path via `Url::path_segments_mut`, so a base URL with or without a trailing slash both work.

`handle_oracle_transaction_proposed` no longer decides anything locally: every proposal for our oracle emits the check effect and lands in `WaitingForDynamicCheck`. A `Verdict::Secure` becomes an approving vote, `Insecure` a denying one, `Inconclusive` no vote at all — the translation from the engine's vocabulary to the contract's happens here and nowhere else. `StaticChecker::Decision` and its `reason` string disappear: `WaitingForRequest`'s `reason` is derived from `RuleId::code()` at the single place `handle_dynamic_check_result` already does it. `bindings.rs`'s `From<&SafeTransaction> for safe_tx::types::SafeTransaction` stays, now with exactly one call site (building the effect) instead of two.

### Other test coverage

- **Moved checkers keep their tests verbatim.** `static_checker.rs`, `cow.rs`, and `address_poisoning.rs` are all `#[cfg(test)]`-inline and move with their modules; only `CheckOutcome` -> `Verdict` spellings change. `address_poisoning.rs` continues to use `ProviderBuilder::connect_mocked_client` + `Asserter`.
- **`crates/sentinel`** adapts `dynamic_checker.rs`'s existing one-shot-`TcpListener` tests onto `engine.rs`: secure, insecure with a cited rule, inconclusive, unrecognized rule code, non-`200`, unreachable endpoint, and timeout.
- **Flow tests** in `service.rs` keep driving `Message::Resume` directly, so they gain no network dependency; they change only where they assert an inline static denial (Phase 4) and where they build the service's components (Phases 5-8).
- **Integration**: `run_sentinel_integration_test.sh` gains a real engine per sentinel, so the existing assertion (both sentinels agree, fees and bonds settle) becomes an end-to-end proof that the split works over HTTP.

---

## Implementation Phases

| Phase | Summary                                                                                                            | Depends on | Own PR |
| ----- | ------------------------------------------------------------------------------------------------------------------ | ---------- | ------ |
| 1     | `safe-tx`: serde for `RuleId` via its Charter code, plus `wire.rs`'s JSON transaction types                        | —          | Yes    |
| 2     | `crates/sentinel-engine`: crate, `openapi.yaml`, config, `axum` server, wire types + golden tests, Dockerfile + CI | 1          | Yes    |
| 3     | Sentinel: adopt the spec's `Verdict` in place of `CheckOutcome` (pure retype)                                      | 2          | Yes    |
| 4     | Sentinel: `StaticChecker` becomes a chain member; the FSM always defers                                            | 3          | Yes    |
| 5     | Engine: `Checker` trait + chain runner, with `AddressPoisoningChecker` moved in as its first implementor           | 2, 4       | Yes    |
| 6     | Sentinel: `EngineClient` with a mandatory URL; devnet and integration scripts run one engine per sentinel          | 5          | Yes    |
| 7     | Move `CowChecker` into the engine                                                                                  | 6          | Yes    |
| 8     | Move `StaticChecker` into the engine; the sentinel's chain collapses to the client                                 | 6          | Yes    |
| 9     | Docs: `AGENTS.md`, `README.md`, `docs/devnet.md`, new `docs/sentinel-engine.md`                                    | 8          | Yes    |
| 10    | Remove this plan                                                                                                   | 9          | Yes    |

Phases 7 and 8 touch disjoint files and can be developed in parallel once Phase 6 is in; they only need to be merged in some order. Phases 3 and 4 touch only `crates/sentinel` and can proceed in parallel with Phase 5's engine-side work, modulo Phase 5's small sentinel-side removal. Everything else is sequential. Phase 2 is the one to review most carefully — `openapi.yaml` is the interface every later phase is built against.

### Phase 1 — `safe-tx`: the JSON forms

Two files in `crates/safe-tx`, no other crate touched. Written before the spec that documents them, since the spec has to describe what these types actually emit.

- `rule.rs`: manual `Serialize` (`serialize_str(self.code())`) and `Deserialize` (`from_code`, with an error naming the code that was not recognized) rather than derives, so the Charter citation stays the single wire form and `code`/`from_code` stay the single place the mapping lives. Extends the existing round-trip test to also cover JSON.
- `wire.rs` (new): the hand-written `SafeTransaction`/`Operation` JSON types, `deny_unknown_fields`, addresses emitted checksummed via a `#[serde(with = "...")]` helper and decoded case-insensitively, `U256`/`Bytes` left to `alloy`. Conversions against `safe_tx::types` in both directions, with asymmetric signatures, because the two types genuinely differ in what they can hold: `From<wire::SafeTransaction> for types::SafeTransaction` is infallible, while the other direction is a `TryFrom` that fails on `types::Operation::__Invalid` — the `sol!` escape hatch the wire type deliberately has no room for. The sentinel is the one caller that can hit it (an event carrying an out-of-range operation byte), and treats it as `Inconclusive` with an `error` log: such a transaction cannot execute on a Safe anyway, so there is nothing to deny and no reason to guess. Carries the wire-format tests listed above.

Note the module docs on `types.rs` should point at `wire.rs` and say why there are two encodings of the same transaction — the next person to touch either will otherwise reasonably assume one of them is redundant.

### Phase 2 — `crates/sentinel-engine`: the service and its contract

New crate: `Cargo.toml`, `openapi.yaml`, `main.rs`, `config.rs` (`bind_address` + `observability` only at this point), `api.rs` (router, handler, `CheckRequest`/`Verdict`, golden-JSON tests, router tests), `Dockerfile`, `Dockerfile.dockerignore`, plus the workspace `Cargo.toml` and `.github/workflows/docker.yml` entries.

No checkers yet: the handler answers `Inconclusive` for every request. That is a truthful answer from an engine with nothing configured to check, and it keeps this PR about the contract and the transport rather than about verification logic. It also means no `Checker` trait is introduced ahead of its first implementor (Phase 5). Nothing on the sentinel side changes.

Also adds `@redocly/cli` (or equivalent) as a root dev-dependency with an `npm run check:openapi` script wired into the root `check` script, so a malformed or self-inconsistent spec fails CI the same way a Biome or `clippy` violation does.

### Phase 3 — Sentinel: adopt `Verdict`

Mechanical retype, no behavior change. `crates/sentinel/src/checker.rs` replaces `CheckOutcome` with the spec's `Verdict` shape; `Checker::check` returns it; `effect.rs`, `cow.rs`, `address_poisoning.rs`, `dynamic_checker.rs`, `service.rs` and their tests follow the rename (`Approved` -> `Secure`, `Denied(rule)` -> `Insecure { rule }`, `Unknown` -> `Inconclusive`). Where the sentinel is talking about its vote rather than about the transaction, the `approve`/`deny` wording stays — the point of the rename is that those are now two different things. `dynamic_checker.rs`'s ad-hoc `Request`/`Response` structs are reshaped to the spec's `CheckRequest`/`Verdict` over `safe_tx::wire` and gain the golden-JSON tests, which also deletes its "denied without a recognized rule code" runtime branch — that case is now a deserialization failure.

### Phase 4 — Sentinel: the static check joins the chain

The only behavior-changing phase on the sentinel's FSM, kept separate from every move so it can be reviewed alone.

- `static_checker.rs`: `StaticChecker` implements `Checker` and returns `Verdict`; its `check` takes `&safe_tx::types::SafeTransaction` instead of `&bindings::consensus::SafeTransaction`; `Decision` is deleted.
- `service.rs`: `handle_oracle_transaction_proposed` stops calling the checker inline and always emits `Effect::DynamicCheck`, landing the request in `WaitingForDynamicCheck`. `StaticChecker` moves into the chain built in `Service::components`, ahead of `CowChecker`.
- `SentinelTransition` loses its `static_checker` field; `SentinelService` keeps it only to hand to the chain.
- Flow tests: the blocklist-denial cases stop asserting an immediate inline state and instead resolve the effect, like every other case already does.
- Observable change: a blocklisted destination is now denied one effect-resolution later than before. Under the prerequisite epic, that resolution is a background task rather than an inline await, so nothing else is held up while it happens.

### Phase 5 — Engine: the checker chain, with its first check

Engine side: `checker.rs` (the `Checker` trait plus the chain runner, moved out of the sentinel's `effect.rs`), `address_poisoning.rs` (moved verbatim), the `rpc` and `[engine] address_poisoning_lookback_blocks` config fields, and the handler switching from a constant `Inconclusive` to running the chain.

Sentinel side: `address_poisoning.rs` deleted, dropped from the chain in `Service::components`, its field removed from `SentinelService`/`main.rs`, and `address_poisoning_lookback_blocks` removed from `SentinelConfig` (plus the two scripts that set it). `checker.rs` stays for now — the sentinel still has a chain (`StaticChecker`, `CowChecker`, `RemoteChecker`).

`AddressPoisoningChecker` is deliberately the first checker moved, because it is the only one that never returns `Insecure`: it either answers `Secure` off unforgeable evidence of a prior interaction, or abstains. While it is dormant (Phases 5-6), the sentinel's chain simply falls through to the next checker more often, and no vote it casts differs from what it would have cast before.

### Phase 6 — Sentinel: talk to the engine

- `dynamic_checker.rs` -> `engine.rs`; `RemoteChecker` -> `EngineClient` with a mandatory base URL, an explicit timeout, and the `x-request-id` header; its tests adapted and extended with the timeout and non-`200` cases.
- `SentinelConfig`: `remote_check_url` -> a `[sentinel.engine]` table (`url`, `timeout`). `crates/sentinel/Dockerfile`'s runtime-stage comment, which names `remote_check_url` as one of the two reasons `ca-certificates` is installed, is updated with it.
- `scripts/run_sentinel_integration_test.sh`: `cargo build --package sentinel-engine` alongside the sentinel, an engine config and `cargo run` per sentinel on distinct ports, `engine.url` set in each sentinel config, and engine logs captured. `.github/workflows/integration.yml`'s log-upload list gains the engine logs.
- `scripts/run_devnet.sh`: an `engine-<name>` container per sentinel in the pod spec (distinct `bind_address` ports, since pod containers share a network namespace), a generated `engine.toml` per sentinel, `engine.url` in each sentinel config, and the new image in `--build`.

After this phase the split is live end to end: address poisoning is checked again, over HTTP, in the engine.

### Phase 7 — Move `CowChecker`

`cow.rs` moved verbatim into `crates/sentinel-engine` (module path and `use` lines only), appended to the engine's chain after `StaticChecker`, and dropped from the sentinel's. Large diff, but a rename-only one: review with `git diff -M`. No new config (CoW's addresses and API base URL are compile-time constants today, and stay that way).

### Phase 8 — Move `StaticChecker`; the sentinel's chain disappears

`static_checker.rs` moved verbatim into the engine and placed at the head of its chain; `blocklist` moves from `[sentinel]` to `[engine]` (and in both scripts). On the sentinel side, `checker.rs` is deleted outright, and `effect::Handler` collapses from a `Vec<Box<dyn Checker>>` to a single `EngineClient` call. `crates/sentinel` now contains no transaction-verification logic.

### Phase 9 — Docs

`AGENTS.md`'s Rust crate list, `README.md`'s Project Organisation list, `docs/devnet.md` (container names, `--build` image list, the config-inspection commands that read `/config/sentinel.toml`), and a new `docs/sentinel-engine.md`: what the engine is, its config reference, how to run one, and a pointer to `openapi.yaml` as the interface contract for operators writing their own. Two things the prose has to get right for that operator audience, since both are easy to mis-read off the reference implementation: the invariants are "no key, no bond, no onchain writes" — persistence is not on that list, and an engine of their own is welcome to keep a database — and a verdict is about the transaction (`secure`/`insecure`), while approving and denying are the sentinel's vote, which is what its bond is staked on.

### Phase 10 — Remove this plan

Delete `epics/2026_08_07_sentinel_engine.md`.

---

## Open Questions and Assumptions

- **`epics/2026_07_24_nonblocking_effects.md` lands first.** This is a hard prerequisite, not a preference: Phases 4 and 6 together put a network round trip on the path of every proposal, which must not run inline under the transition mutex. Phase 4 also assumes `WaitingForDynamicCheck` exists. Phases 1-3 are independent of it and could be merged earlier, though there is little reason to.
- **`CheckRequest`/`Verdict` are duplicated between the two crates on purpose**, with `openapi.yaml` and a golden-JSON fixture on each side as the anti-drift mechanism. The transaction itself is not duplicated — `safe_tx::wire` owns it, since both crates already depend on `safe-tx` and the JSON encoding of a Safe transaction is that crate's subject. If a third Rust consumer of this API ever appears, extracting a shared crate for the envelope is a mechanical follow-up; it is not worth doing for two.
- **A rule citation the sentinel does not recognize resolves to `Inconclusive`.** The Charter's rule set is open (the spec says so), but a build's `RuleId` is closed, so an engine running a newer `safe-tx` than its sentinel can cite a rule the sentinel cannot parse, and the sentinel then declines to vote on a transaction its engine considers insecure. That is the right failure — a vote is a bonded claim, and relaying a citation you cannot reason about is worse than abstaining — but it is a real operational hazard, so: the sentinel logs it at `error` (naming the code), and the upgrade order is engine-after-sentinel. The alternative, making the sentinel's wire type carry the citation as an opaque string and pass it through to `reveal`'s `reason`, is worth revisiting if operators start running engines on an independent release cadence; it is not worth the loss of type safety while both binaries ship from this repo at the same version.
- **The chain-id/RPC mismatch in `AddressPoisoningChecker` is knowingly carried over, not fixed.** The engine gets a single `rpc` endpoint, so its onchain lookups read from whichever chain that is, which is not necessarily `transaction.chainId`. `openapi.yaml` documents `chainId` as the chain the transaction targets and does not claim the engine verifies against it. Fixing this properly (a chain-id-to-RPC map, and rejecting transactions for unconfigured chains) is a small follow-up epic that is much easier once the engine owns both the RPC configuration and the verification logic. Flagged rather than silently inherited.
- **No authentication between sentinel and engine.** The assumption is that the two are co-deployed (same pod, same host, loopback or a private network), exactly as the devnet models. Anyone who can reach the engine can ask it questions, and the answers are not secret. If an operator ever wants to run a shared or remote engine, that needs an auth scheme and probably rate limiting; the spec leaves room for it (a security scheme can be added without touching the request or response shape) but does not specify one.
- **Neither service records metrics today**, so none are added here. The engine reuses `safenet_core::observability` for logging and the Prometheus listener, which is enough to see that it is up. Per-verdict counters and a latency histogram are the obvious first metrics for the whole repo, and are left to whichever change introduces metrics generally.
- **`Inconclusive` remains a drop, not a retry.** A sentinel that gets no trustworthy verdict declines to vote and lets the request's deadline sweep it, exactly as today. Retrying a failed check (with backoff, bounded by the commit deadline) is a plausible improvement and an independent one: it lives entirely inside the sentinel's effect handling and needs nothing from this split.
- **Exact Rust shapes are intentionally left loose** — the `EngineClient` constructor signature, how `api.rs` splits handler from router, field ordering — to be settled in implementation and PR review rather than gated on this document, consistent with the preceding epics.
- **`axum`'s current major version** is to be confirmed at implementation time (`0.8` as of writing) and added to `[workspace.dependencies]` like every other shared dependency.
