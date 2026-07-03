# Plan: Commit-Reveal Voting for the Sentinel Game (V2)

Component: `contracts/src/SentinelOracle.sol` + its libraries
(`contracts/src/libraries/SentinelOracleRequests.sol`,
`contracts/src/libraries/SentinelOracleCommitments.sol`), and the Rust sentinel client
(`crates/sentinel`).

---

## Overview

The `SentinelOracle` contract runs the "sentinel game": once a Safe transaction is proposed,
registered sentinels race to publicly bond a vote (`commitApprove`/`commitDeny`) for or against it
before a single `VOTING_WINDOW` deadline. Votes are visible the instant they land onchain. This
epic replaces that public-vote race with a two-phase **commit-reveal** scheme: sentinels first lock
a bond behind a blind hash of their vote, then reveal the actual vote once the commit window closes.

This is a direct implementation of a tech spec written before the current contract existed (see the
task description). Two parts of that spec no longer apply and are dropped rather than carried
forward (see [Architecture Decision](#architecture-decision)):

- **Section 5.3 ("Resolving Unmet Thresholds Post-Reveal")** describes an aggregate `$B_req` capital
  threshold model. The deployed contract never worked that way — it resolves on **unanimity**
  (every bonded checker on one side, none on the other) with a **fixed per-checker bond**
  (`bondTarget = fee * bondMultiplier`), not an aggregate capital target. V2 keeps the unanimity
  model; "unmet threshold" becomes "nobody revealed" or "only one side revealed," both of which map
  directly onto the existing `TIMED_OUT` / `RESOLVED_*` outcomes.
- **Section 6 ("Asymmetric Bonds & Commit-Reveal Integration")** solves a "capital leak" that only
  exists if `B_yes != B_no`. The deployed contract has always used a single symmetric `bondTarget`
  for both sides. There is no capital leak, no need for a "Shadow Lock," and no new protocol-wide
  bond cap — the existing `bondMultiplier` governance knob is untouched.

**Scope.** This epic changes the onchain protocol and its one active production client, the Rust
`crates/sentinel` binary. The TypeScript sentinel (`validator/src/sentinel/`) is **not** ported to
V2 — see the [client-scope note](#client-scope-the-ts-sentinel-is-retired-not-ported) below for why,
and the consequence it has for the existing integration test suite (Phase C).

The work splits into:

- **Phase A** — contract changes: commit-reveal state machine, hashing, non-reveal slashing, tests.
- **Phase B** — Rust sentinel changes: two-phase FSM, salt derivation, updated bindings.
- **Phase C** — fix the integration test suite (it currently drives the now-incompatible TS
  sentinel) and remove this plan file.

---

## Architecture Decision

### Two independent, fixed-length windows

Replace the single `VOTING_WINDOW` immutable with two: `COMMIT_WINDOW` and `REVEAL_WINDOW` (spec
§5.1's `t_commit`/`t_reveal`). Both are fixed at contract construction, matching the existing
`VOTING_WINDOW` immutable pattern — no dynamic/threshold-based early cutoff of the commit phase is
introduced (V1 never had one either; see
[Open Questions and Assumptions](#open-questions-and-assumptions)).

A request's `revealDeadline` is set once, at creation time, as `commitDeadline + REVEAL_WINDOW`.
This keeps the two timers decoupled per §5.1 (a congested reveal window doesn't shrink because the
commit window is unaffected, and vice versa) while avoiding a second onchain state transition to
"open" the reveal phase — the deadlines alone determine which phase a block number falls into.

### Reusing the existing unanimity + fixed-bond model, with equal-share rewards

The bond a checker locks (`bondTarget = fee * bondMultiplier`) does not change and is identical
regardless of vote, exactly as today. Locking it during `commit()` therefore leaks nothing about the
vote — the "Capital Leak" problem in spec §6 requires `B_yes != B_no`, which this contract has never
had.

- `commit(requestId, commitHash)` locks `bondTarget`, stores the hash, and increments a
  side-blind `committedCount`/`totalCommittedBond`.
- `reveal(requestId, approve, salt)` verifies the hash and adds the bond to whichever side's
  `approveSentinelCount`/`totalApproveBond` or `denySentinelCount`/`totalDenyBond` it belongs to.
- `finalize()` keeps V1's exact three-way unanimity check
  (`FROZEN` if both sides > 0, `RESOLVED_*` if only one side > 0, `TIMED_OUT` if neither), just
  fed from **revealed** totals instead of committed totals.

**Reward math changes, and simplifies, on team decision: every revealer on the winning side receives
an equal share of the fee (`fee / winningSideCount`), regardless of reveal order.** V1's
position-weighted score (`bondAmount * 1e18 / position`, rewarding earlier voters more) is dropped
for V2. It no longer made sense to preserve anyway — "earlier" only meant anything when votes were
public and racing; under commit-reveal, reveal order carries no information about who did the
underlying validation work first, so weighting by it wouldn't reward what it used to. Dropping it
also removes `approveTotalScore`/`denyTotalScore` bookkeeping entirely, which is the priority for a
first version. `resolveDispute` and `isBondSlashed` need no logic changes; `calcFeeReward` gets
simpler (see Tech Specs); only the two commitment libraries and `SentinelOracle`'s external functions
otherwise change.

### Commit hash binds `(approve, salt, sentinel, requestId)`

The hash preimage is `keccak256(abi.encodePacked(approve, salt, msg.sender, requestId))`, not just
`keccak256(abi.encodePacked(approve, salt))`. Both bindings are load-bearing, not defense-in-depth:

- **`requestId`** closes a cross-request leak: a sentinel that reuses the same `(approve, salt)` pair
  across two concurrent requests would otherwise produce **identical commit hashes** for both —
  visibly correlating its vote across requests before either is revealed, which defeats blindness
  just as effectively as a plaintext vote would for the second request.
- **`msg.sender`** closes a same-request replay that would otherwise gut the whole point of
  commit-reveal (independent validation). Without it: sentinel B copies sentinel A's opaque
  `commitHash` verbatim into its own `commit()` call during the commit window — B needs no knowledge
  of A's vote to do this, since the hash reveals nothing on its own. Once A reveals `(approve, salt)`,
  B replays those exact values into its own `reveal()` call; since the hash was copied, it validates,
  and B has "voted" identically to A without doing any of the underlying work — herding, just done by
  copying hashes instead of copying votes. Binding `msg.sender` makes A's `commitHash` and revealed
  `(approve, salt)` unusable for anyone else's commitment, since the preimage no longer matches once
  the sender differs.

### Deterministic salt derivation via HMAC-SHA256 (no salt persisted offchain, no new secret)

The naive approach — generate a random salt per vote and persist it until reveal — creates a new
failure mode: if the sentinel's local DB is lost or corrupted between commit and reveal, the salt is
gone and the bond is unrecoverably slashed for "failure to reveal" even though the node did its job
and voted.

Instead, the salt is derived deterministically from the sentinel's own account key:

```
salt = HMAC-SHA256(key = private_key_bytes, message = "safenet-sentinel-reveal-salt" || requestId)
```

There is no new secret — the sentinel already holds the private key for its onchain account
(`safenet-core::tx::Signer`), and the salt is a pure, deterministic function of that key plus
`requestId`, so nothing needs to be persisted; it's recomputed on demand when `reveal()` is
submitted, naturally robust to restarts, reorgs, and DB loss.

An earlier draft of this plan derived the salt via RFC 6979 §3.2 (the deterministic-nonce
construction behind deterministic ECDSA). Review feedback settled on plain HMAC-SHA256 instead: RFC
6979 exists to produce a scalar inside the elliptic curve's group order `[1, q-1]`, which needs a
rejection-sampling/reduction step that only matters when the output is actually used as an ECDSA
nonce. The salt here is never used as a nonce — it's an opaque 32-byte value the contract only ever
feeds into `keccak256(abi.encodePacked(approve, salt, msg.sender, requestId))` — so that machinery
buys nothing. A bare, un-keyed hash (e.g. `keccak256(privateKey || requestId)`) was considered and
rejected too, in favor of a real HMAC construction: HMAC-SHA256 is the standard, audited primitive
for "deterministic pseudorandom output keyed by a secret," and a raw concatenate-and-hash is a less
rigorous substitute for that exact use case.

`safenet-core::tx::Signer` gains a new method implementing this (keeping the raw private-key bytes
encapsulated there rather than exposed to the sentinel crate, consistent with how it already owns all
signing), using the `hmac`/`sha2` crates already in the dependency tree transitively via the existing
signing stack. The `"safenet-sentinel-reveal-salt"` prefix is domain-separation hygiene — it keeps
this derivation's input space clearly distinct from any other use of the same key — but unlike the
RFC 6979 draft it is not a hard safety requirement, since HMAC's security doesn't degrade under
reuse the way ECDSA nonce reuse does (see [Open Questions](#open-questions-and-assumptions)).

### Non-reveal slashing: one uniform step in `finalize()`

Per spec §5.2, a checker who commits but never reveals gets slashed (otherwise a griefer can hit the
bond target and then withhold their reveal to stall the request indefinitely). The chosen mechanics:

- `finalize()` computes `unrevealedBond = totalCommittedBond - totalApproveBond - totalDenyBond`
  once revealed totals are final (see the early-finalize rule below), **regardless of which of the
  three outcomes follows**, and transfers it to `ARBITRATOR` — mirroring the existing
  `resolveDispute()` pattern where the arbitration loser's slashed bond goes to `ARBITRATOR`. No new
  distribution logic is introduced. This transfer must happen **before** V1's existing
  `if (newState == State.FROZEN) { return; }` early exit in `SentinelOracle.finalize()` (see Tech
  Specs) — a conflicted, disputed request can still have non-revealing committers on either side, and
  that early return must not skip slashing them.
- The proposer's fee refund is unaffected: it already only happens on `TIMED_OUT`, funded from the
  separate `fee` balance the proposer paid — not from checker bonds. There's nothing to "top up"
  from the slashed pool in the `RESOLVED_*` branches, since the fee there is paid out to the winning
  side exactly as in V1.
- `claim()` must reject any commitment still at `Vote.PENDING` (new `NotRevealed` error). Without
  this guard a non-revealer could still call `claim()` after their bond was already swept out by the
  `finalize()` slash step above, incorrectly minting a second payout from the contract's remaining
  balance.

### Early finalization when everyone has revealed

`finalize()` becomes callable once `block.number > revealDeadline` **or**
`revealedCount == committedCount` (tracked the same way `approveSentinelCount`/`denySentinelCount`
are today), whichever comes first — matching the spec's sequence diagram and avoiding the full
reveal-window latency in the common case where every committer reveals promptly. A request with zero
commits can resolve as soon as `commitDeadline` passes, without waiting for `revealDeadline` at
all, since there is nothing to reveal.

### Client scope: the TS sentinel is retired, not ported

`validator/src/sentinel/` calls `commitApprove`/`commitDeny`, both of which this epic removes from
the contract's ABI. Porting it to commit-reveal was considered and rejected for this epic: the Rust
port (`epics/2026_06_25_rust_sentinel_port.md`) is nearly complete (only its own Phase F —
interop test + docs cleanup — remains) and is already the intended long-term replacement, so
duplicating the commit-reveal FSM in a client about to be retired is wasted work. **Consequence:**
after this epic, the TS sentinel can no longer participate in a `SentinelOracle` dispute at all, and
the rust-port epic's planned "TS/Rust interoperate in a shared dispute" interop test (its F1) is no
longer meaningful — it should be narrowed to a Rust-only integration test. The existing integration
suite (`scripts/run_integration_test.sh` → `npm test -w validator -- integration`) currently drives
the TS sentinel against a deployed `SentinelOracle` and will fail outright once the contract changes
land; Phase C fixes this by switching that script's sentinel leg to the Rust binary. See
[Open Questions](#open-questions-and-assumptions).

### Alternatives Considered

- **Shadow Lock (over-collateralize every commit to the max of `B_yes`/`B_no`).** Not needed —
  bonds are already symmetric, so there is no "max" to shadow-lock against.
- **A protocol-wide bond cap, independent of the fee-scaled `bondTarget`.** Rejected per the
  answered scoping question: bonds stay exactly as they are today (`fee * bondMultiplier`,
  identical for both sides); no new governance parameter is introduced.
- **Slash remainder distributed pro-rata to honest revealers (whistleblower-style) instead of to
  `ARBITRATOR`.** Rejected per the answered scoping question, in favor of reusing the existing
  `resolveDispute()` payout target and avoiding new distribution accounting. Can be revisited later
  as a separate, self-contained change if the incentive design calls for it.
- **Randomly-generated, persisted salt instead of deterministic derivation.** Rejected — it adds a
  DB-loss failure mode (an honest vote becomes an unrevealable, slashed one) for no benefit over a
  key-derived salt.
- **A new, separately-configured `reveal_secret`, hashed with `requestId`
  (`keccak256(reveal_secret || requestId)`).** Rejected — needs a second secret to generate,
  configure, and protect alongside the signing key, for no benefit over deriving from the key the
  sentinel already holds.
- **Full RFC 6979 §3.2 deterministic-nonce generation.** The original design here, superseded on
  review: RFC 6979 is built to produce a valid elliptic-curve scalar (rejection sampling + reduction
  mod the curve order), which is unneeded complexity for an opaque 32-byte salt that's never used as
  an actual ECDSA nonce. Plain HMAC-SHA256 gives the same "deterministic, unpredictable without the
  key" property with far less machinery.
- **A bare, un-keyed hash of the private key and `requestId`** (e.g. `keccak256(privateKey ||
  requestId)`). Rejected on review in favor of a proper HMAC construction — HMAC-SHA256 is the
  standard, audited primitive for this exact "keyed deterministic PRF" use case.
- **Keep V1's position-weighted reward (`bondAmount * 1e18 / position`), just computed at
  reveal-time instead of commit-time.** Rejected per the team's decision to prioritize simplicity for
  this first version: every revealer on the winning side gets an equal share of the fee, dropping
  `approveTotalScore`/`denyTotalScore` entirely. Weighting by reveal order also stopped meaning what
  it used to (see [Architecture Decision](#reusing-the-existing-unanimity--fixed-bond-model-with-equal-share-rewards)),
  so nothing valuable is lost by simplifying.
- **Open the reveal phase with an explicit onchain transition/event instead of a precomputed
    `revealDeadline`.** Rejected — an extra state transition buys nothing here since both window
  lengths are fixed at request creation; the deadlines alone are enough to gate `commit`/`reveal`.
- **Threshold-based early end of the commit phase** (spec §5.1: "or when the maximum required bond
  threshold is theoretically reached"). Not implemented — V1 never had an early commit cutoff either
  (`finalize()` always required the fixed window to elapse), and blind commitments make a
  bond-target-based cutoff meaningless anyway (the contract can't know which side a blind commit
  will land on). Flagged as an assumption, not a decision to revisit within this epic.
- **Port the TS sentinel to commit-reveal alongside the Rust one.** Rejected — see
  [Client scope](#client-scope-the-ts-sentinel-is-retired-not-ported) above.

---

## Tech Specs

### `SentinelOracleCommitments.sol`

```solidity
enum Vote {
    PENDING,   // committed, not yet revealed
    APPROVED,
    DENIED
}

struct Commitment {
    bytes32 commitHash;
    uint256 bondAmount;
    Vote vote;
    bool claimed;
}
```

An enum replaces the original `bool revealed; bool approved;` pair (per review feedback) — it rules
out the otherwise-representable, meaningless state of `approved` holding a stale value while
`revealed == false`, and every call site reads more directly (`vote == PENDING` instead of
`!revealed`).

- `add(requestId, sentinel, commitHash, bondAmount)` — replaces today's `add(..., approve, ...)`;
  sets `vote = PENDING`; emits `Committed(requestId, sentinel, bondAmount)` (no vote — that's not
  known yet).
- New `reveal(requestId, sentinel, approve)` — requires `vote == PENDING`, then sets
  `vote = approve ? APPROVED : DENIED`; emits `Revealed(requestId, sentinel, approved, bondAmount)`.
  No `position` — reward is an equal split among winning revealers (see Architecture Decision), so
  nothing needs to be assigned at reveal time beyond which side the vote landed on.
- `checkNotCommitted` unchanged in spirit, now checks `commitHash == 0`.
- `markClaimed` gains `require(self.vote != Vote.PENDING, NotRevealed())` before its existing checks.

### `SentinelOracleRequests.sol`

```solidity
struct Request {
    address proposer;
    uint256 fee;
    uint256 bondTarget;
    uint256 commitDeadline;   // was `deadline`
    uint256 revealDeadline;   // new: commitDeadline + REVEAL_WINDOW, set at creation
    State state;
    uint256 committedCount;       // new: commits regardless of side
    uint256 totalCommittedBond;   // new: sum of all locked bonds, revealed or not
    uint256 revealedCount;        // new: successful reveals so far
    uint256 totalApproveBond;
    uint256 totalDenyBond;
    uint256 approveSentinelCount;
    uint256 denySentinelCount;
}
```

`approveTotalScore`/`denyTotalScore` are dropped — reward is an equal split of `fee` among revealers
on the winning side, so only the winning side's count is needed (see `calcFeeReward` below).

- `applyCommit(self)` — commit-phase bookkeeping only (no `approve` param): requires
  `state == PENDING && block.number <= commitDeadline`, locks `bondTarget`, increments
  `committedCount`/`totalCommittedBond`. Returns `bondAmount` for the caller to pull.
- `applyReveal(self, approve)` — requires `block.number > commitDeadline && block.number <=
  revealDeadline`; increments `revealedCount` and the appropriate side's `*SentinelCount`/
  `total*Bond`. No return value — bond amount was already fixed and recorded at commit time.
- `finalize(self)` — requires
  `state == PENDING && (block.number > revealDeadline || (committedCount > 0 && revealedCount ==
  committedCount) || committedCount == 0 && block.number > commitDeadline)`; computes
  `unrevealedBond = totalCommittedBond - totalApproveBond - totalDenyBond` and returns it alongside
  the existing `(newState, refundFee)` so the caller can transfer it to `ARBITRATOR`. The three-way
  unanimity check itself is unchanged from V1.
- `calcFeeReward(self, approved)` — simplified: returns `0` if `approved` lost or the request timed
  out, else `fee / winningSideCount` (`approveSentinelCount` or `denySentinelCount`, whichever side
  won). No longer takes `bondAmount`/`position`.
- `resolveDispute`, `isBondSlashed` — unchanged.
- New errors: `CommitWindowClosed`, `RevealWindowNotOpen`, `RevealWindowClosed`.

### `SentinelOracle.sol`

- Constructor: replace `votingWindow` with `commitWindow` and `revealWindow`; both stored as new
  immutables `COMMIT_WINDOW`/`REVEAL_WINDOW` (drop `VOTING_WINDOW`).
- `postRequest`: `commitDeadline = block.number + COMMIT_WINDOW`; `revealDeadline = commitDeadline +
  REVEAL_WINDOW`; both passed to `$requests.create`.
- Replace `commitApprove`/`commitDeny` with a single `commit(bytes32 requestId, bytes32 commitHash)`.
- New `reveal(bytes32 requestId, bool approve, bytes32 salt)`:
  `require(keccak256(abi.encodePacked(approve, salt, msg.sender, requestId)) ==
  commitment.commitHash, InvalidReveal())`, then applies `applyReveal` and records the reveal.
- New pure helper `hashCommitment(address sentinel, bytes32 requestId, bool approve, bytes32 salt)
  external pure returns (bytes32)` so tests (and, if useful, offchain tooling) don't hand-roll the
  packing — single source of truth for the preimage layout.
- `finalize()`: after computing `(newState, refundFee, unrevealedBond)`, transfer `unrevealedBond` to
  `ARBITRATOR` whenever it is non-zero, **before** the existing
  `if (newState == State.FROZEN) { return; }` early exit. Getting the ordering wrong here would
  silently skip the non-reveal slash for every disputed request. In addition to existing per-outcome
  behavior otherwise.
- `claim()`: unchanged call shape; relies on the library's new `NotRevealed` guard.
- `DeploySentinelOracle.s.sol`: replace `SENTINEL_VOTING_WINDOW` with `SENTINEL_COMMIT_WINDOW` /
  `SENTINEL_REVEAL_WINDOW` env vars.

### `crates/sentinel`

- `bindings.rs`: regenerate `sol!` bindings for `commit`/`reveal`/`hashCommitment`, the new
  `Committed`/`Revealed` event shapes, and `NewRequest` carrying `commitDeadline`/`revealDeadline`.
- `hashing.rs`: `commit_hash(sentinel, request_id, approve, salt)` mirroring the Solidity preimage
  exactly, plus `reveal_salt(signer, request_id)` — calls the new `Signer` method (see below) with a
  domain-separated message derived from `request_id` and returns its HMAC-SHA256 output as the salt.
  Parity-tested against `SentinelOracle.hashCommitment` (extends the existing parity-vector pattern
  used for `request_id`) and against RFC 4231 HMAC-SHA256 test vectors for the underlying primitive.
- `safenet-core::tx::signer.rs`: new method (e.g. `reveal_salt(message: B256) -> B256`) computing
  `HMAC-SHA256(private_key_bytes, message)` over the wrapped `SigningKey`'s raw key material, using
  the `hmac`/`sha2` crates already in the dependency tree. No new config field — the salt is derived
  from the account's existing private key, not a separately configured secret.
- `state.rs`: `RequestStatus` gains two stages between `Committed` and `Finalized`: `Revealing` (our
  `Reveal` action has been emitted, not yet confirmed onchain) and `Revealed` (our own `Revealed`
  event confirmed it). These must be two distinct statuses, not one — `handle_block_advance` runs on
  *every* new block, so if "waiting for the commit deadline to pass" and "reveal already submitted,
  awaiting confirmation" shared a status, the FSM would have no way to tell it already emitted the
  `Reveal` action and would re-emit a duplicate `Reveal` transaction on every subsequent block until
  the confirming event round-trips (at least one block later). `SentinelRequestState` tracks
  `commitDeadline`/`revealDeadline` instead of a single `deadline`; the salt is **not** stored (see
  [Architecture Decision](#architecture-decision)). `SentinelActionKind::CommitApprove`/`CommitDeny`
  collapse into `Commit { id, hash }`; new `Reveal { id, approve, salt }`.
- `service.rs`:
  - `handle_new_request` computes `commit_hash` via `hashing::commit_hash` and emits a single
    `Commit` action (status → `Pending`, same as today).
  - `handle_committed` (our own `Committed` event) moves `Pending → Committed`, exactly as V1 — it
    does **not** yet emit the `Reveal` action, since revealing before `commitDeadline` is invalid
    onchain.
  - `handle_block_advance` gains the phase transition V1 didn't need, and each branch moves the
    request **out of** the status it matches on in the same step, so it cannot match again on a later
    block and re-emit:
    - `Committed` request, `block > commitDeadline` → emit `Reveal` (deriving the salt on the fly)
      and move to `Revealing`. A `Revealing` request is otherwise left untouched by block advances —
      it only progresses via `handle_revealed` below, never by re-matching this branch.
    - `Revealed` request, `block > revealDeadline` → emit `Finalize` and move to `Finalized`, exactly
      as V1 did from `Committed`.
  - New `handle_revealed` (our own `Revealed` event) moves `Revealing → Revealed`.
  - `handle_resolved` unchanged in intent: claim iff we reached `Revealed`/`Finalized` and either
    timed out or our revealed vote matches the outcome.
- Unit tests mirror the existing style in `service.rs`/`state.rs`/`hashing.rs`/`action.rs` for every
  new branch above.

### Testing

- `contracts/test/SentinelOracle.t.sol`: rewrite the existing flows (unanimous approve/deny,
  no-commitments timeout, conflict → dispute) to go through `commit`+`reveal`, and add: reveal before
  `commitDeadline` reverts, reveal after `revealDeadline` reverts, wrong hash/salt reverts,
  double-commit/double-reveal reverts, early `finalize` once all committers revealed, partial-reveal
  (some committed, fewer revealed) still resolves correctly, and a non-revealer's bond is slashed to
  `ARBITRATOR` while their own `claim()` reverts with `NotRevealed`.
- `crates/sentinel`/`safenet-core`: unit tests for the new FSM transitions (including that
  `handle_block_advance` never emits a second `Reveal`/`Finalize` for a request already moved out of
  the matching status), the `Signer`'s HMAC-SHA256 salt derivation (including RFC 4231 test vectors,
  not just self-consistency), the resulting `commit_hash`/`reveal_salt`, and `encode_actions` for
  `Commit`/`Reveal`.
- Integration: see Phase C.

---

## Implementation Phases

Each PR has a single purpose, targets < 300 changed LOC and < 10 files, and is independently
reviewable. "Depends on" lists hard ordering; everything else may proceed in parallel.

### Phase A — Contracts (blocks Phase B; parallel-safe internally is limited since all three touch
the same small library set)

- **A1 — Commitment library: blind commit + reveal.** Rewrite `SentinelOracleCommitments.sol`
  (struct, `add`, new `reveal`, `NotRevealed` guard in `markClaimed`) per Tech Specs. No caller
  changes yet (library-only PR).
- **A2 — Request library: two-phase deadlines, equal-share reward, unrevealed-bond accounting.**
  Rewrite `SentinelOracleRequests.sol` (`commitDeadline`/`revealDeadline`, `committedCount`/
  `totalCommittedBond`/`revealedCount`, `applyCommit`/`applyReveal` split, simplified
  `calcFeeReward`, updated `finalize` signature/logic). Depends on A1 (shares the `Commitment` shape
  referenced by callers, though the library itself doesn't import it — sequencing avoids two
  half-migrated libraries mid-review).
- **A3 — Wire `SentinelOracle.sol` + deploy script.** `commit`/`reveal`/`hashCommitment` external
  functions, updated constructor/immutables (`COMMIT_WINDOW`/`REVEAL_WINDOW`), `finalize()`'s
  unrevealed-bond transfer to `ARBITRATOR`, `DeploySentinelOracle.s.sol` env var rename. Depends on
  A1, A2.
- **A4 — Foundry tests.** Full rewrite/extension of `SentinelOracle.t.sol` per the Testing section.
  Depends on A3 (needs the finished external interface to exercise).

### Phase B — Rust sentinel (depends on Phase A merged; see the coordination note in Open Questions
about sequencing against the in-flight rust-port epic)

- **B0 — `safenet-core::tx::Signer`: HMAC-SHA256 deterministic salt.** New method computing
  `HMAC-SHA256(private_key_bytes, message)` over the wrapped `SigningKey`, plus unit tests against
  RFC 4231 HMAC-SHA256 test vectors. No sentinel-specific code yet — this is a generic, reusable
  primitive on the shared core crate. Can start immediately (no dependency on Phase A).
- **B1 — Bindings & hashing.** `bindings.rs` regeneration + `hashing.rs` (`commit_hash`,
  `reveal_salt`) with parity tests against `hashCommitment`. Depends on Phase A (needs the final
  ABI) and B0 (`reveal_salt` calls the new `Signer` method).
- **B2 — State & actions.** `state.rs` (`RequestStatus::{Revealing,Revealed}`, dual deadlines) and
  `action.rs` (`Commit`, `Reveal`) per Tech Specs, with tests. Depends on B1 (uses `commit_hash`'s
  return type).
- **B3 — Service FSM.** `service.rs`: the full commit → wait → reveal → wait → finalize → claim
  transition rewrite, with unit tests for every new branch. Depends on B2.

### Phase C — Fix the integration suite & wrap up

- **C1 — Point the integration test at the Rust sentinel.** `scripts/run_integration_test.sh`:
  replace the `SENTINEL_VOTING_WINDOW` env var with the two new ones, and swap the sentinel leg from
  `npm test -w validator -- integration` (TS) to building/running the `crates/sentinel` binary
  against the deployed contract, since the TS sentinel can no longer complete a vote. Depends on
  Phase A (deploy script) and Phase B (a working Rust client).
- **Cleanup.** Per the planning convention, **remove this epic file**
  (`epics/2026_07_02_sentinel_commit_reveal_voting.md`) once C1 is merged and green.

### Critical path

`A1 → A2 → A3 → A4 → B1 → B3 → C1`. B0 can start as soon as this plan is approved (no dependency on
Phase A) and just needs to land before B1.

---

## Open Questions and Assumptions

**Open questions**

1. **Sequencing against `epics/2026_06_25_rust_sentinel_port.md`.** That epic's own Phase F (interop
   test + docs/cleanup) is still open. If it lands first, Phase B here is a clean continuation. If
   it's still in flight when Phase A merges, Phase B will conflict with it in `bindings.rs`/
   `service.rs` — recommend landing the rust-port epic's Phase F first, or explicitly coordinating a
   rebase. Either way, that epic's planned "TS/Rust interoperate in a shared dispute" interop test
   should be narrowed to a Rust-only test once this epic lands, since TS can no longer participate
   (see [Client scope](#client-scope-the-ts-sentinel-is-retired-not-ported)).
2. **Fate of `validator/src/sentinel/`.** This epic leaves that code in the tree but non-functional
   against the new contract (it still calls the removed `commitApprove`/`commitDeny`). Deleting it
   outright is a natural follow-up but is left as a separate, focused cleanup PR rather than folded
   into this epic, since it's unrelated to the commit-reveal mechanism itself.
3. **`COMMIT_WINDOW`/`REVEAL_WINDOW` values.** Picking concrete block counts for each (and for the
   existing `bondMultiplier`) is a deployment/config decision, not a contract-shape decision — left
   to whoever configures the next deployment, same as `VOTING_WINDOW` is today.
4. **Exact domain-separation prefix for the HMAC message.** B0 needs a concrete constant (e.g.
   `"safenet-sentinel-reveal-salt"`) folded in alongside `requestId`. This is hygiene rather than a
   hard security requirement — HMAC's security doesn't degrade under key reuse across messages the
   way ECDSA nonce reuse does, unlike the RFC 6979 draft this superseded — but it costs nothing and
   keeps this derivation's input space clearly separate from any other future use of the same key.

**Assumptions**

- No threshold-based early end of the commit phase is introduced; both windows are fixed-length,
  matching V1's fixed `VOTING_WINDOW`.
- Bonds stay symmetric (`bondTarget = fee * bondMultiplier` for both sides); no new protocol-wide
  bond cap.
- Non-reveal slashing always sends the full unrevealed pool to `ARBITRATOR`; the proposer's existing
  `TIMED_OUT` fee refund is unaffected and unrelated to that pool.
- Reward for winning revealers is an equal split of `fee` (no position/bond weighting); this is a
  deliberate behavior change from V1, chosen for simplicity in this first version.
- The Rust sentinel derives its reveal salt deterministically via HMAC-SHA256 from the account's
  existing signing key and a domain-separated message containing `requestId` — no new secret is
  generated or configured, and no salt is persisted in the snapshot store.
- Only `crates/sentinel` is ported to commit-reveal in this epic; `validator/src/sentinel/` is left
  as-is (and effectively retired) rather than ported.
