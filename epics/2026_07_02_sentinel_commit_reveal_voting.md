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

### Non-reveal slashing: only when an established side exists to prove it against

Per spec §5.2, a checker who commits but never reveals gets slashed **if doing so is provably
griefing** — otherwise a griefer could hit the bond target and then withhold their reveal to stall
the request indefinitely. Slashing requires an *established side* to grief against: if
`approveSentinelCount > 0 || denySentinelCount > 0`, a committer who never revealed had a chance to
help resolve the request (their own commit already contributed to the outcome) and chose not to —
that's the griefing case. But if **nobody** reveals at all (`TIMED_OUT`), there is no established
side and no way to distinguish an honest reveal-infrastructure failure from malice; slashing
everyone's bond in that case punishes committers for something that isn't provably misbehavior. The
chosen mechanics:

- `finalize()` computes `unrevealedBond = totalCommittedBond - totalApproveBond - totalDenyBond`
  once revealed totals are final (see the early-finalize rule below), **for the `FROZEN`/
  `RESOLVED_APPROVED`/`RESOLVED_DENIED` outcomes only**, and transfers it to `ARBITRATOR` —
  mirroring the existing `resolveDispute()` pattern where the arbitration loser's slashed bond goes
  to `ARBITRATOR`. No new distribution logic is introduced. This transfer must happen **before**
  V1's existing `if (newState == State.FROZEN) { return; }` early exit in `SentinelOracle.finalize()`
  (see Tech Specs) — a conflicted, disputed request can still have non-revealing committers on either
  side, and that early return must not skip slashing them. On `TIMED_OUT`, `unrevealedBond` is zero:
  nobody's bond is slashed, so every committer can reclaim it in full via `claim()`.
- The proposer's fee refund is unaffected: it already only happens on `TIMED_OUT`, funded from the
  separate `fee` balance the proposer paid — not from checker bonds. There's nothing to "top up"
  from the slashed pool in the `RESOLVED_*` branches, since the fee there is paid out to the winning
  side exactly as in V1.
- `claim()` must require a resolved outcome. For every other resolved outcome, the fee and bonds to
  return should be calculated. In case of a `TIMED_OUT` all funds return to the original parties.

### Early finalization when everyone has revealed

`finalize()` becomes callable once `block.number > revealDeadline` **or**
`revealedCount == committedCount` (tracked the same way `approveSentinelCount`/`denySentinelCount`
are today), whichever comes first — matching the spec's sequence diagram and avoiding the full
reveal-window latency in the common case where every committer reveals promptly. A request with zero
commits can resolve as soon as `commitDeadline` passes, without waiting for `revealDeadline` at
all, since there is nothing to reveal.

### Client FSM: phases mirror the oracle's protocol states, not our own action lifecycle

V1's Rust client FSM (`crates/sentinel/src/state.rs`) models its states around *our own* action
lifecycle (`Preparing -> Pending -> Committed -> Finalized`, i.e. "have we submitted this action
yet, has it confirmed"). A straightforward port of that shape to V2 would just insert one more
submit/confirm pair for the reveal (`Committed -> Revealing -> Revealed`). V2 instead restructures
the client's per-request state around the **oracle's own protocol phases** — `WaitingForRequest`,
`CollectingCommitments`, `CollectingVotes`, `WaitingForDisputeResolution` — mirroring
`SentinelOracleRequest.State` directly, with per-node bookkeeping (our vote intent, whether our own
commit/reveal landed, running commit/reveal tallies) as plain fields on each phase rather than
additional FSM states. This is the same rationale as the `Vote` enum replacing `bool revealed; bool
approved` in the Solidity commitment struct: a phase's fields should only be representable when they
make sense (e.g. `approve_count`/`deny_count` don't exist before there's anything to reveal), and a
request's state should read as "where is this in the protocol," not "what has *this node* personally
done about it."

This restructuring changes client behavior in three ways beyond the state names:

- **Early finalization is client-driven, not just contract-permitted.** `CollectingVotes` tracks
  `revealed_count`/`committed_count` (and per-side `approve_count`/`deny_count`) from every
  `Committed`/`Revealed` event, not just our own — the same tallies the oracle keeps onchain. The
  moment `revealed_count == committed_count`, the client finalizes immediately rather than waiting
  out `REVEAL_WINDOW`, taking full advantage of the early-finalization rule above instead of just
  being compatible with it.
- **The no-dispute happy path claims without waiting for `OracleResult`.** Because resolution is
  unanimous, a sentinel with its own revealed vote counted knows the outcome the moment the other
  side's tally is (and stays) zero: there is no `TIMED_OUT` case to consider (that requires zero
  votes on *both* sides, impossible once our own vote is one of them), and if the other side has any
  votes at all it's a dispute, not a loss. So `Finalize` and `Claim` are emitted together, from
  locally computed data, and the request is dropped immediately — no round trip through our own
  `OracleResult` event. This relies on the `TransactionQueue` preserving submission order for
  same-account actions (already relied on for `ApproveToken` before `Commit`), so `Finalize` mines
  before the dependent `Claim`.
- **`Finalize` fires without an open vote of our own only on a genuine timeout
  (`revealed_count == 0`).** A gate of "only finalize if I revealed" would match V1's
  `Committed`-regardless-of-outcome unconditional finalize for the happy path, but silently drops
  liveness for the case where every committer fails to reveal: no client in the fleet would have an
  open vote to justify finalizing, yet `finalize()` performing the non-reveal slash (see above)
  depends on someone actually calling it. That liveness gap only exists when *nobody* revealed —
  once at least one sentinel's reveal lands, that sentinel's own FSM reaches this same step with
  `self_revealed == true` and finalizes; a sentinel that never revealed has nothing to claim and
  would just be spending gas redundantly alongside it. So `Finalize` is emitted whenever
  `self_revealed` is `true` (as before), and additionally when `self_revealed` is `false` but
  `revealed_count == 0` — the case no revealer's FSM exists to cover. `Claim` stays conditional on
  our own reveal having landed (`self_revealed`), since a `claim()` without a matching revealed
  commitment reverts with `NotRevealed`.

Only when the local tally shows both sides nonzero (`FROZEN`, i.e. a genuine dispute) does the client
still need to wait for an external signal — `resolveDispute()`'s `OracleResult` (`ARBITRATION`
reason) — since that outcome is decided by the arbitrator, not derivable from anything the client
already knows.

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
- **Model the Rust client FSM around our own action lifecycle**, i.e. V1's shape carried straight
  over to V2 (`Preparing → Pending → Committed → Revealing → Revealed → Finalized`, one submit/
  confirm pair per action). Superseded by the phase-oriented model in
  [Client FSM](#client-fsm-phases-mirror-the-oracles-protocol-states-not-our-own-action-lifecycle):
  a pure action-lifecycle FSM only ever knows about our own actions, so it can't drive early
  finalization or the immediate-claim happy path, and the all-committers-fail-to-reveal liveness gap
  has to be patched on after the fact rather than falling out of the design.

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
- `state.rs`: replace the single `RequestStatus` tag + flat `SentinelRequestState` with one enum,
  `SentinelRequestState`, whose variants carry only the fields meaningful in that phase — mirroring
  `SentinelOracleRequest.State` (see the new
  [Client FSM](#client-fsm-phases-mirror-the-oracles-protocol-states-not-our-own-action-lifecycle)
  Architecture Decision) instead of our own action-submit/confirm lifecycle:
  - `WaitingForRequest { approve, deadline }` — `deadline` is our own guessed cutoff (the real
    `commitDeadline` isn't known until `NewRequest` arrives), same role as V1's `Preparing`.
  - `CollectingCommitments { approve, commit_deadline, reveal_deadline, committed_count,
    self_committed }` — `committed_count` tallies every `Committed` event (any sentinel);
    `self_committed` tracks whether *our own* commit was among them.
  - `CollectingVotes { approve, reveal_deadline, committed_count, revealed_count, approve_count,
    deny_count, self_revealed }` — `committed_count` is the snapshot carried over from the previous
    phase (no more commits are possible once this phase is entered); the rest tally every `Revealed`
    event the same way.
  - `WaitingForDisputeResolution { approve }` — only our own vote needs to survive into this phase,
    to compare against the eventual arbitration outcome.
  The salt is still **not** stored anywhere (see
  [Architecture Decision](#deterministic-salt-derivation-via-hmac-sha256-no-salt-persisted-offchain-no-new-secret)).
- `action.rs`: `SentinelActionKind::CommitApprove`/`CommitDeny` collapse into `Commit { id, hash }`;
  new `Reveal { id, approve, salt }`.
- `service.rs`:
  - `handle_oracle_transaction_proposed` unchanged in spirit: seeds `WaitingForRequest`.
  - `handle_new_request` (`WaitingForRequest` only) computes `commit_hash`, emits `Commit`, and moves
    to `CollectingCommitments { committed_count: 0, self_committed: false, .. }`.
  - `handle_committed` (`CollectingCommitments` only, any sentinel): increments `committed_count`;
    additionally sets `self_committed = true` when `event.sentinel == self.account`.
  - New `handle_revealed` (`CollectingVotes` only, any sentinel): increments `revealed_count` and the
    matching `approve_count`/`deny_count`; sets `self_revealed = true` when
    `event.sentinel == self.account`; then, if `revealed_count == committed_count`, immediately runs
    the finalize step below (early finalization — see Architecture Decision) instead of waiting for
    `handle_block_advance`.
  - `handle_block_advance`:
    - `WaitingForRequest`, `block > deadline`: drop (unchanged from V1's `Preparing` timeout).
    - `CollectingCommitments`, `block > commit_deadline`: if `self_committed`, emit `Reveal` (salt
      derived on the fly) and move to `CollectingVotes { revealed_count: 0, approve_count: 0,
      deny_count: 0, self_revealed: false, committed_count, .. }`; otherwise drop the request (our
      own commit never landed onchain, so revealing would just revert).
    - `CollectingVotes`, `block > reveal_deadline`: run the finalize step below.
  - Shared finalize step (reached from either the early-finalize check in `handle_revealed` or the
    deadline branch in `handle_block_advance`, always leaving `CollectingVotes` in the same step so
    it cannot run twice for one request):
    - `self_revealed == false`, `revealed_count == 0` (genuine timeout, no revealer's FSM exists to
      finalize instead) → emit `Finalize` **and** `Claim` and drop. Unlike every other
      `self_revealed == false` case, `claim()` succeeds here: a `TIMED_OUT` outcome never slashes
      unrevealed bonds (see the Non-reveal slashing decision above), so our own still-`PENDING`
      commitment is claimable for its full bond.
    - `self_revealed == false`, `revealed_count > 0` → drop without emitting anything; whichever
      sentinel did reveal finalizes from its own FSM instead.
    - `self_revealed == true`, `approve_count > 0 && deny_count > 0` (dispute) → emit `Finalize`,
      move to `WaitingForDisputeResolution { approve }`.
    - `self_revealed == true`, otherwise → emit `Finalize` and `Claim` (unanimity plus our own
      counted vote guarantees we're on the sole, winning side; no `OracleResult` round trip needed)
      and drop.
  - `handle_resolved` (our own `OracleResult`, `WaitingForDisputeResolution` only): decode the
    `ResolveReason`, emit `Claim` iff `event.approved == approve`, and drop the request either way.
- Unit tests mirror the existing style in `service.rs`/`state.rs`/`hashing.rs`/`action.rs` for every
  new branch above, plus the early-finalization path and the timeout-only `Finalize`-plus-`Claim`
  liveness case.

### Testing

- `contracts/test/SentinelOracle.t.sol`: rewrite the existing flows (unanimous approve/deny,
  no-commitments timeout, conflict → dispute) to go through `commit`+`reveal`, and add: reveal before
  `commitDeadline` reverts, reveal after `revealDeadline` reverts, wrong hash/salt reverts,
  double-commit/double-reveal reverts, early `finalize` once all committers revealed, partial-reveal
  (some committed, fewer revealed) still resolves correctly with a non-revealer's bond slashed to
  `ARBITRATOR` while their own `claim()` reverts with `NotRevealed`, and a pure timeout (some or all
  committed, nobody revealed) refunding every committer's bond in full via `claim()` (no slash, no
  `NotRevealed`) alongside the proposer's fee refund.
- `crates/sentinel`/`safenet-core`: unit tests for the new FSM transitions (including that a request
  can only reach its finalize step once — via either the early-finalize check in `handle_revealed` or
  the deadline branch in `handle_block_advance`, never both), the tally bookkeeping
  (`committed_count`/`revealed_count`/`approve_count`/`deny_count` incrementing for *any* sentinel,
  not just our own), the timeout-only `Finalize`-plus-`Claim` liveness branch (and that a non-revealer
  emits nothing once someone else's reveal has landed), the
  dispute-vs-immediate-claim split out of `CollectingVotes`, the `Signer`'s HMAC-SHA256 salt
  derivation (including RFC 4231 test vectors, not just self-consistency), the resulting
  `commit_hash`/`reveal_salt`, and `encode_actions` for `Commit`/`Reveal`.
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

### Phase B — Rust sentinel (depends on Phase A merged and on
`epics/2026_06_25_rust_sentinel_port.md` having already landed — see Assumptions)

- **B0 — `safenet-core::tx::Signer`: HMAC-SHA256 deterministic salt.** New method computing
  `HMAC-SHA256(private_key_bytes, message)` over the wrapped `SigningKey`, plus unit tests against
  RFC 4231 HMAC-SHA256 test vectors. No sentinel-specific code yet — this is a generic, reusable
  primitive on the shared core crate. Can start immediately (no dependency on Phase A).
- **B1 — Bindings & hashing.** `bindings.rs` regeneration + `hashing.rs` (`commit_hash`,
  `reveal_salt`) with parity tests against `hashCommitment`. Depends on Phase A (needs the final
  ABI) and B0 (`reveal_salt` calls the new `Signer` method).
- **B2 — State & actions.** `state.rs` (the `SentinelRequestState` phase enum:
  `WaitingForRequest`/`CollectingCommitments`/`CollectingVotes`/`WaitingForDisputeResolution`) and
  `action.rs` (`Commit`, `Reveal`) per Tech Specs, with tests. Depends on B1 (uses `commit_hash`'s
  return type).
- **B3 — Service FSM.** `service.rs`: the full propose → commit → wait → reveal → wait → finalize →
  claim transition rewrite (including the early-finalization and unconditional-non-reveal-finalize
  branches), with unit tests for every new branch. Depends on B2.

  Landed as a `servicev2.rs` module kept **alongside** (not replacing) `service.rs`, mirroring B1/B2's
  V1-plus-V2-coexisting convention — each sub-task PR stays independently reviewable without touching
  the live V1 FSM that `main.rs` still runs. Further split into sub-tasks, landing in order:
  - **B3a** (this PR) — the `SentinelService`/`SentinelTransition`/`SentinelEncoder` skeleton: struct
    definitions, the full V2 action encoder, and the `apply_transition` event matcher dispatching to
    handler stubs (`todo!()`). No handler logic and no tests yet — nothing to test until B3b starts
    filling handler bodies in.
  - **B3b** — `handle_oracle_transaction_proposed`/`handle_new_request` (blind `commit_hash` +
    `Commit`, the `WaitingForRequest` → `CollectingCommitments` transition).
  - **B3c** — `handle_committed`/`handle_revealed` (commit/reveal tallying, the shared finalize step,
    and early finalization).
  - **B3d** — `handle_block_advance` (the `WaitingForRequest` timeout, the `CollectingCommitments`
    deadline branch emitting `Reveal`, and the `CollectingVotes` deadline branch running the finalize
    step).
  - **B3e** — Fix Oracle behavior to return funds to all parties in case of a timeout.
  - **B3f** — `handle_resolved` for `WaitingForDisputeResolution`.
  - **B3g** — Flow test. Instead of individual unit tests for each transition function, tests for complete flow (triggering multiple state transitions) will be implemented for the service.

### Phase C — Fix the integration suite & wrap up

- **C1 — Point the integration test at the Rust sentinel.** `scripts/run_integration_test.sh`:
  replace the `SENTINEL_VOTING_WINDOW` env var with the two new ones, and swap the sentinel leg from
  `npm test -w validator -- integration` (TS) to building/running the `crates/sentinel` binary
  against the deployed contract, since the TS sentinel can no longer complete a vote. Depends on
  Phase A (deploy script) and Phase B (a working Rust client).

  Also wires `main.rs` onto `servicev2::SentinelService` (threading the config `Signer` into it,
  replacing the `service::SentinelService` wiring) — the integration test can't exercise the V2 FSM
  otherwise. Deliberately kept out of B3: `main.rs` must keep running the V1 FSM until there's a
  working V2 client to switch to, so the integration suite (still V1-only until this phase) never
  breaks mid-epic.
- **C2 — Retire V1.** Once C1 is green, delete `service.rs`, the V1 `RequestStatus`/
  `SentinelRequestState`/`State` in `state.rs`, the V1 `SentinelActionKind`/`SentinelAction` in
  `action.rs`, and the V1 `SentinelOracle` contract/`SentinelEvents` in `bindings.rs`; rename the
  `V2`-suffixed survivors (`SentinelRequestStateV2` → `SentinelRequestState`, `SentinelOracleV2` →
  `SentinelOracle`, `servicev2.rs` → `service.rs`, etc.) to their canonical, unsuffixed names. Depends
  on C1 (only delete V1 once V2 is proven wired and green in the integration suite).
- **Cleanup.** Per the planning convention, **remove this epic file**
  (`epics/2026_07_02_sentinel_commit_reveal_voting.md`) once C2 is merged and green.

### Critical path

`A1 → A2 → A3 → A4 → B1 → B3 → C1 → C2`. B0 can start as soon as this plan is approved (no dependency
on Phase A) and just needs to land before B1.

---

## Open Questions and Assumptions

No open questions remain — the sequencing and value-picking items from earlier drafts turned out to
be decisions rather than open ones, and are folded into the assumptions below.

**Assumptions**

- `epics/2026_06_25_rust_sentinel_port.md` — including its own Phase F (interop test + docs cleanup)
  — will be finished before this epic's Phase B starts. Phase B is therefore a clean continuation,
  with no cross-epic conflict or rebase coordination needed in `bindings.rs`/`service.rs`. That
  epic's planned "TS/Rust interoperate in a shared dispute" interop test should be narrowed to a
  Rust-only test once this epic lands, since TS can no longer participate (see
  [Client scope](#client-scope-the-ts-sentinel-is-retired-not-ported)).
- `validator/src/sentinel/` is left in the tree, non-functional against the new contract, for the
  duration of this epic (only `crates/sentinel` is ported to commit-reveal here). Removing the TS
  sentinel is out of scope: it's deleted in a separate epic once the broader Rust validator port
  (`epics/2026_07_01_rust_validator_port.md`) is complete and the whole TS validator is retired
  together, not as a standalone cleanup tied to this one.
- Concrete `COMMIT_WINDOW`/`REVEAL_WINDOW` values (and the existing `bondMultiplier`) are a
  deployment/config decision, left to whoever configures the next deployment — same as `VOTING_WINDOW`
  is today, not something this plan needs to pin down.
- A domain-separation prefix (e.g. `"safenet-sentinel-reveal-salt"`) is folded into the HMAC message
  alongside `requestId` in B0; the exact wording is an implementation detail, not a blocking decision
  — it's hygiene rather than a hard security requirement, since HMAC's security doesn't degrade under
  key reuse across messages the way ECDSA nonce reuse does.
- No threshold-based early end of the commit phase is introduced; both windows are fixed-length,
  matching V1's fixed `VOTING_WINDOW`.
- Bonds stay symmetric (`bondTarget = fee * bondMultiplier` for both sides); no new protocol-wide
  bond cap.
- Non-reveal slashing only sends the unrevealed pool to `ARBITRATOR` when an established side exists
  to grief against (`FROZEN`/`RESOLVED_*`); a pure `TIMED_OUT` (nobody revealed) can't distinguish
  griefing from an honest reveal-infrastructure failure, so nothing is slashed and every committer
  reclaims their bond in full via `claim()`. The proposer's existing `TIMED_OUT` fee refund is
  unaffected and unrelated to that pool either way.
- Reward for winning revealers is an equal split of `fee` (no position/bond weighting); this is a
  deliberate behavior change from V1, chosen for simplicity in this first version.
- The Rust client calls `finalize()` once its local tally says doing so is valid (either
  `revealed_count == committed_count` or past `revealDeadline`), even for a request where it never
  itself revealed, but only when `revealed_count == 0` too (nobody revealed at all). That's the one
  case with no other sentinel's FSM to rely on for liveness — otherwise no client in the fleet would
  ever call `finalize()`. If someone else did reveal, their own FSM finalizes instead, so a
  non-revealer skips it rather than redundantly spending gas on a call it gets nothing from. `Claim`
  stays conditional on the client's own reveal having landed **or** the request having timed out —
  a `TIMED_OUT` outcome never slashes unrevealed bonds (see the Non-reveal slashing decision), so a
  still-`PENDING` commitment is claimable there, unlike every other outcome.
- The Rust sentinel derives its reveal salt deterministically via HMAC-SHA256 from the account's
  existing signing key and a domain-separated message containing `requestId` — no new secret is
  generated or configured, and no salt is persisted in the snapshot store.
