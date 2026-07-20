# Plan: Sentinel Vote Reason

Component: `contracts` (Solidity) and `crates/sentinel` (Rust).

---

## Overview

Sentinels currently reveal a bare `approve`/`deny` vote (plus the blind-commit `salt`) on `SentinelOracleV2`. There's no way to record *why* a sentinel voted the way it did, which makes disputes and post-hoc audits harder than they need to be (e.g. "why did this sentinel deny this request?" currently has no onchain answer). This epic adds a `reason` string to the reveal step, folded into the blind commit hash (so it's locked in at commit time, not writable after seeing others reveal), and surfaced in the `Revealed` event.

Two PRs, sequential:

- **Phase A** — Solidity: add `reason` to `reveal()`, `hashCommitment()`, `computeHash()` and the `Revealed` event. Includes the minimal Rust-side ABI mirror needed to keep `crates/sentinel` compiling and the `sentinel-integration` CI job (`.github/workflows/integration.yml`) green — hardcoded to an empty `reason` — since that job builds and runs the actual sentinel binary against the freshly deployed contract.
- **Phase B** — Rust: extend `Detector` to produce a real reason and wire it through the FSM, replacing Phase A's hardcoded empty string.

The `validator/src/sentinel/` TypeScript implementation is legacy (superseded by `crates/sentinel`; last touched at #519, frozen since the "Sentinel Oxidation" rewrite landed) and is out of scope.

---

## Architecture Decision

`reason` is a plain `string`, folded into the blind commit hash alongside `approve` (`SentinelOracleCommitment.computeHash`, mirrored by `hashing::commit_hash`, becomes `keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason))` — `reason` appended last since it's the only variable-length field in the packed encoding, keeping it unambiguous). It is still **not** written to contract storage — the `SentinelOracleCommitment.Commitment` struct is unchanged; the hash check at reveal time is enough to prove the revealed `reason` matches what was committed, without persisting the text onchain.

This follows from the stronger guarantee you asked for: `reason` should be locked in blind, at commit time, exactly like `approve` — a sentinel can no longer see how others voted (or what they wrote) before finalizing their own justification, and can't tweak their wording after the fact to match/copy an earlier revealer. The tradeoff is real and worth being explicit about:
- **No revising `reason` after committing.** A typo or an omission can't be fixed later — the reveal must resupply the exact committed string, or it reverts with `InvalidReveal()` (same failure mode as a wrong `approve`/`salt` today).
- **`reason` must be decided deterministically alongside `approve`, once.** `Detector::decide` is called once at proposal time and its output (`approve` + `reason`) is carried unchanged through state to both the `commit` hash computation and the later `reveal` call — it must never be re-derived independently at reveal time, since even a whitespace difference would brick the reveal.
- **The hash preimage is now variable-length.** `hashing::commit_hash`'s current implementation packs a fixed 85-byte array by hardcoded offset; it has to change to build the preimage dynamically to fit an arbitrary-length `reason`.

Rolling out is split so the Rust crate never has a window where it's out of sync with the deployed contract: Phase A lands the Solidity change *and* the minimal Rust touch needed to keep building valid commit/reveal pairs against it — the ABI mirror in `bindings.rs` unavoidably changes shape (it mirrors the contract's actual function/event signatures), but nothing else does. `hashing::commit_hash` and `SentinelActionKind::Reveal` keep their current signature/shape; the encoder simply encodes a literal `""` for `reason` where the ABI now requires it. Phase B is what introduces `reason` as a real, threaded value — adding it as a parameter to `commit_hash`, a field on `Reveal`, and fields on `SentinelRequestState`, sourced from `Detector::decide`. Nothing in Phase A is "half-finished" pending Phase B: it's a complete, correct implementation of "always reveal with an empty reason," which Phase B then extends.

### Alternatives Considered

- **Leave `reason` out of the commit hash (event-only, freeform at reveal time).** This was the initial proposal — simpler (fixed-size hash preimage, no re-derivation-determinism constraint, wording could be fixed up after commit). Rejected per your ask for a stronger guarantee: without binding it, a sentinel can freely rewrite `reason` after seeing earlier reveals in the same reveal window, so it can't prove the justification predates seeing others' votes.
- **Store `reason` in the `Commitment` struct (contract storage).** Rejected: the hash-check at reveal already proves the revealed text matches what was committed; persisting it onchain too would cost an extra SSTORE for no additional guarantee (the `Revealed` event is sufficient for offchain/indexer consumption).
- **A fixed `enum` of reasons instead of a freeform string.** Rejected for now: a freeform string is more flexible for the `Detector`'s (currently blocklist-based) logic to describe itself. Worth reconsidering if reasons need to be machine-parsed later.
- **Land the Solidity change and Rust change in one PR.** Rejected to keep PRs reviewable and single-purpose per file (contracts vs. Rust); instead Phase A carries just enough Rust plumbing (hardcoded `""`) to keep `sentinel-integration` CI green in between, rather than mixing the full `Detector` rewrite into the same PR as the Solidity change.

---

## Tech Specs

### Phase A — Solidity, plus minimal Rust plumbing

Solidity:
- `contracts/src/libraries/SentinelOracleCommitmentsV2.sol`:
  - `SentinelOracleCommitment.computeHash(address sentinel, bytes32 requestId, bool approve, bytes32 salt, string calldata reason)` — `reason` appended last, folded into `keccak256(abi.encodePacked(...))`.
  - `SentinelOracleCommitmentMap.reveal(...)` gains a `string calldata reason` parameter, recomputes the hash with it, and reverts `InvalidReveal()` on any mismatch (wrong `approve`, `salt`, *or* `reason`).
  - `event Revealed(bytes32 indexed requestId, address indexed sentinel, bool approved, uint256 bondAmount, string reason)` — `reason` appended, non-indexed; emitted only once the hash check above has passed, so it's guaranteed to match what was committed.
- `contracts/src/SentinelOracleV2.sol`:
  - `reveal(bytes32 requestId, bool approve, bytes32 salt, string calldata reason)` (`reason` appended after `salt`) forwards to the library.
  - `hashCommitment(address sentinel, bytes32 requestId, bool approve, bytes32 salt, string calldata reason)` gains the same `reason` param, so callers can precompute the exact commit hash offchain before calling `commit()`.
- `contracts/test/SentinelOracleV2.t.sol`:
  - Update the `_commit` and `_reveal` helpers to accept and forward a shared `reason` string (the same value must be used for both, or reveal reverts).
  - Add a test asserting `reason` shows up verbatim on the emitted `Revealed` event.
  - Add a test confirming an empty `reason` is accepted (sentinels shouldn't be forced to justify every vote).
  - Add a test asserting reveal reverts with `InvalidReveal()` when `reason` doesn't match what was committed (same shape as the existing wrong-`salt`/wrong-`approve` revert test around line 340).
- Open question below on whether to cap `reason` length.

Rust (minimal — no new parameters or fields; `crates/sentinel` keeps compiling and `sentinel-integration` CI green against the new ABI by always encoding/hashing an empty `reason`):
- `src/bindings.rs`: mirror the new `reveal`/`hashCommitment` functions and `Revealed` event (`reason: String`) in the `sol!` macro — unavoidable, this just reflects the contract's actual ABI.
- `src/hashing.rs`: `commit_hash` keeps its current `(sentinel, request_id, approve, salt)` signature — no new parameter. Internally, adjust the preimage construction (and its doc comment) to explicitly include an empty `reason` component, so it stays an honest mirror of the library's new `computeHash`. Note: since `abi.encodePacked` of an empty string contributes zero bytes, the produced hash is bit-identical to today's — the existing `commit_hash_parity` expected value doesn't need to change.
- `src/action.rs`: unchanged — `SentinelActionKind::Reveal` keeps its current `{ id, approve, salt }` shape.
- `src/service.rs`: only `SentinelEncoder`'s `Reveal` arm changes, encoding a literal `reason: String::new()` into `SentinelOracle::revealCall` — not sourced from the action. `handle_new_request` and `handle_block_advance` are untouched (`commit_hash`'s signature didn't change).
- Explicitly **not** touched in Phase A: `src/state.rs` (no `reason` field on `SentinelRequestState`) and `src/detector.rs` (`Detector::approve` unchanged) — that's Phase B.

### Phase B — Rust: wire the real reason through `Detector` and state

- `src/detector.rs`: replace `Detector::approve(&self, tx: &SafeTransaction) -> bool` with `Detector::decide(&self, tx: &SafeTransaction) -> Decision`, where `Decision { approve: bool, reason: String }` (e.g. `"destination is blocklisted"` / `"destination is not blocklisted"`), called exactly once per request. Update the unit tests accordingly.
- `src/state.rs`: `SentinelRequestState::WaitingForRequest` and `SentinelRequestState::CollectingCommitments` each gain `reason: String` (not `CollectingVotes`/`WaitingForDisputeResolution` — see Architecture Decision). This is load-bearing, not just informational: the exact same `reason` value must flow from `WaitingForRequest` through to both the `commit_hash` call in `handle_new_request` and the `Reveal` action built later in `handle_block_advance`. Existing serde-roundtrip tests need updating for the new field.
- `src/hashing.rs`: `commit_hash` now gains the `reason: &str` parameter Phase A deliberately left out, replacing the hardcoded empty component. The `commit_hash_parity` test needs a new expected value for a non-empty test `reason`, regenerated against the Solidity library (via `forge test`).
- `src/action.rs`: `SentinelActionKind::Reveal` gains `reason: String`.
- `src/service.rs`:
  - `handle_oracle_transaction_proposed`: call `detector.decide(...)` once, store both `approve` and `reason` in the new `WaitingForRequest` fields (replacing Phase A's implicit `""`).
  - `handle_new_request`: carry `reason` from `WaitingForRequest` into `CollectingCommitments`, and pass the carried value (no longer a literal `""`) into `commit_hash(...)`.
  - `handle_block_advance`: read the same `reason` off the `CollectingCommitments` entry when building the `Reveal` action (no longer `String::new()`), so it matches what was hashed at commit time.
  - `SentinelEncoder`: read `reason` off the `Reveal` action instead of hardcoding `String::new()`.
  - Update existing tests that construct `WaitingForRequest`/`CollectingCommitments`/`Reveal` actions or call `commit_hash` (all three shapes are changing) with a `reason` value.

---

## Implementation Phases

### Phase A: Solidity `reveal()` gains a `reason`, Rust kept green (own PR)

Files: `contracts/src/SentinelOracleV2.sol`, `contracts/src/libraries/SentinelOracleCommitmentsV2.sol`, `contracts/test/SentinelOracleV2.t.sol`, `crates/sentinel/src/{bindings,hashing,service}.rs`. No new Rust parameters or struct fields — `bindings.rs` mirrors the new ABI (unavoidable), and the encoder/hashing internals encode/hash a literal empty `reason` so `crates/sentinel` keeps compiling and the `sentinel-integration` CI job stays green against the new contract.

### Phase B: Wire the real reason through the Rust sentinel (own PR, depends on Phase A)

Files: `crates/sentinel/src/{detector,state,hashing,action,service}.rs`. Depends on Phase A being merged. Adds the `reason` parameter/fields Phase A deliberately left out, sourced from `Detector::decide` and carried through state to the `commit_hash` call and the `reveal` transaction.

### Phase C: Remove this plan

Delete `epics/2026_07_20_sentinel_vote_reason.md` once Phase A and B are merged.

---

## Assumptions

- **No length cap on `reason`.** Freeform, sentinel-controlled calldata; it only affects the caller's own gas cost and log storage, not shared state. Agreed to skip a cap for now.
- **`Revealed.reason` field name kept despite `SentinelOracleRequest.ResolveReason`.** The new `reason` param is unrelated to the existing `ResolveReason` enum (why a *request* resolved, e.g. `UNANIMOUS_APPROVE`/`TIMEOUT`) — this one is why a *sentinel* voted the way it did. No actual namespace collision (different scopes); kept as `reason` to minimize changes, with a one-line NatSpec/doc-comment callout in Phase A so reviewers don't conflate the two.
- **`Detector` reason granularity deferred.** The blocklist-based `Detector` only needs two fixed reason strings (blocklisted / not blocklisted) for now. Anything richer (e.g. per-address custom messages) is left to a follow-up once a proper `Detector` is implemented.
- **Determinism is a correctness requirement, not just a nicety.** Because `reason` is committed blind, `Detector::decide`'s output must be reproducible byte-for-byte between commit time and reveal time (the code already only calls it once and carries the result through state, so this should hold naturally — flagged here so Phase B doesn't accidentally re-derive it from the `Detector` a second time at reveal).
