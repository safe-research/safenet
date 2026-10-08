# F-VAL-036 `NonceState::observe` accepts a non-monotonic sequence and rewinds `next_sequence`, inflating the measured nonce capacity

| Field | Value |
| --- | --- |
| Status | QA-done |
| Crate and module | validator, state/preprocess.rs |
| Location | crates/validator/src/state/preprocess.rs:174-194, 226-247 (related: crates/validator/src/state/sign.rs:30-34, crates/validator/src/state/mod.rs:415-463) |
| Severity | Low / Low |
| Certainty | 40% (Critic C-VAL-B; QA may raise) |
| Assumptions involved | A2 |
| Tags | input-validation |

## Claim

`observe` assigns `self.next_sequence = sequence.saturating_add(1)` unconditionally from an event field, with no check that the sequence is at least the one already recorded. The counter is a monotonic quantity - the contract only ever increments it - but the Rust side treats it as an assignment rather than a maximum, so a single stale or forged `Sign` event with a lower sequence permanently lowers it.

The damage is not the rewind itself but its interaction with `available`, which computes remaining capacity in the current chunk as `SEQUENCE_CHUNK_SIZE - offset`. Rewinding `next_sequence` lowers `offset` and so over-reports capacity by the difference, suppressing top-ups; and because `observe`'s `split_off` has already discarded the chunks below the old position, that phantom capacity is over chunks whose entries may be gone. The result is the same class of silent non-participation as F-VAL-030, reached from a different direction. `observe` is called before any authorisation of the event, on every `Sign` for a group whose epoch this validator tracks, and the validator does not check the emitting contract address for coordinator events, so the input is reachable by anything that can produce a `Sign`-shaped log from a watched address.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The sequence is assigned, not maximised, and the same call prunes chunks based on the new value. | E2 | crates/validator/src/state/preprocess.rs:174-194 | excerpt 1 |
| 2 | `available` derives the current chunk's remaining capacity from `next_sequence`'s offset, so a lower offset over-reports. | E2 | crates/validator/src/state/preprocess.rs:234-247 | excerpt 2 |
| 3 | `expected_chunk` also reads `next_sequence`, so a rewind can lower the chunk index a reservation would target. | E2 | crates/validator/src/state/preprocess.rs:222-232 | excerpt 3 |
| 4 | `observe` runs before the event is matched to any tracked session, and before any check on the event's provenance. | E2 | crates/validator/src/state/sign.rs:30-35 | excerpt 4 |
| 5 | The contract-side sequence is a strictly increasing per-group counter, which is the invariant the Rust side fails to assert. | E2 | contracts/src/FROSTCoordinator.sol:530-542 | excerpt 5 |

### Excerpts

**`crates/validator/src/state/preprocess.rs:174-194`**

```rust
impl NonceState {
    /// Returns the nonce secret-store coordinates and advances the observed
    /// sequence and forgets past roots for nonce chunks that can no longer used
    /// for signing.
    ///
    /// Returns `None` if there is no linked nonce for `sequence`.
    pub(super) fn observe(&mut self, sequence: u64) -> Option<NonceIndex> {
        let (chunk, offset) = preprocess::decode_sequence(sequence);
        let nonce = self
            .chunks
            .get(&chunk)
            .copied
            .flatten
            .map(|root| NonceIndex { root, offset });

        self.next_sequence = sequence.saturating_add(1);
        let (next_chunk, _) = preprocess::decode_sequence(self.next_sequence);
        self.chunks = self.chunks.split_off(&next_chunk);

        nonce
    }
```

**`crates/validator/src/state/preprocess.rs:234-247`**

```rust
    /// Counts canonical and pending nonce capacity from `next_sequence`.
    fn available(&self) -> u64 {
        let (chunk, offset) = preprocess::decode_sequence(self.next_sequence);
        self.chunks
            .range(chunk..)
            .map(|(key, _)| {
                if *key == chunk {
                    SEQUENCE_CHUNK_SIZE.saturating_sub(offset)
                } else {
                    SEQUENCE_CHUNK_SIZE
                }
            })
            .sum
    }
```

**`crates/validator/src/state/preprocess.rs:222-232`**

```rust
    /// Returns the chunk the onchain contract is expected to assign to the
    /// next commitment.
    ///
    /// Returns `None` in case we've reached the very last chunk.
    fn expected_chunk(&self) -> Option<u64> {
        let (chunk, _) = preprocess::decode_sequence(self.next_sequence);
        match self.chunks.last_key_value {
            Some((last, _)) => last.checked_add(1).map(|next| next.max(chunk)),
            None => Some(chunk),
        }
    }
```

**`crates/validator/src/state/sign.rs:30-35`**

```rust
        let nonce = state
            .epochs
            .values_mut
            .find(|epoch| epoch.group.id == event.gid)
            .and_then(|epoch| epoch.nonces.observe(event.sequence));
        match (nonce, state.signing.remove(&event.message)) {
```

**`contracts/src/FROSTCoordinator.sol:530-542`**

```solidity
    function sign(FROSTGroupId.T gid, bytes32 message) external returns (FROSTSignatureId.T sid) {
        require(message != bytes32(0), InvalidMessage);
        Group storage group = $groups[gid];
        GroupState memory state = group.state;
        require(state.count > 0, GroupNotInitialized);
        require(state.status == GroupStatus.FINALIZED, GroupNotReady);
        uint64 sequence = state.sequence++;
        sid = FROSTSignatureId.create(gid, sequence);
        Signature storage signature = $signatures[sid];
        group.state = state;
        signature.message = message;
        emit Sign(msg.sender, gid, message, sid, sequence);
    }
```

## Trigger

A `Sign` event carrying a sequence lower than the validator's current `next_sequence`, for a group whose epoch it tracks. On an honest canonical chain this cannot happen - the log stream is delivered in `(block, index)` order and a reorg rolls the state back together with the chain (`crates/core/src/state/mod.rs:171-189`, `205-215`). It becomes reachable through the coordinator-event provenance gap tracked as VAL-H2/CORE-H4: `Event::Coordinator(...)` is dispatched without comparing `log.address` against the coordinator, and the watched address set includes every operator-configured oracle contract, so a contract at one of those addresses emitting a `Sign`-shaped log with `sequence = 0` sets `next_sequence` to 1 in one transition.

Concretely: with the group at sequence 8000 (chunk 7, offset 832) and one linked chunk 7, `available` correctly returns 192, which is above the 100 threshold but close to it. One injected `Sign` with `sequence = 7168` (chunk 7, offset 0) resets `next_sequence` to 7169, and `available` now reports 1023. The validator will not top up for another 831 real signatures, and every `Sign` beyond the end of chunk 7 finds no linked chunk.

Class: `E2` for the missing bound and the arithmetic; `I` for exploitability, because it inherits VAL-H2's precondition, which is owned by R6 and is itself conditional.

## Considered and rejected

- **"A reorg produces this without any injection."** Checked and rejected. `handle_update` rolls the snapshot back on `BlockUpdate::Uncle` before replaying, so `next_sequence` is restored to a value consistent with the branch being replayed (`crates/core/src/state/mod.rs:171-178`).
- **"Out-of-order log delivery produces this."** Rejected: the core state machine rejects an update whose logs are not strictly sorted and in range (`crates/core/src/state/mod.rs:199-204`).
- **"The rewind lets an attacker replay a burned nonce."** Rejected, and this is the important negative result. A rewind can make `observe` return coordinates for an offset that has already been consumed, but `take_nonce` then returns `None` and the transition no-ops (`crates/validator/src/secrets/store.rs:205-218`, `crates/validator/src/service/effect.rs:189-201`). The failure is fail-closed, which is why this is Low and not a nonce-reuse finding.
- **"`expected_chunk` is broken by the rewind too."** Only mildly: `last.checked_add(1).map(|next| next.max(chunk))` keeps the higher of the tracked maximum and the rewound chunk (basis 3), so a reservation still lands at or above the last tracked chunk. The chunk index only diverges from the contract when the tracked map is empty, which needs a rewind plus fully pruned chunks.
- **Not folded into VAL-H2.** The provenance gap is one root cause; the absence of a monotonicity check is an independent local defect, cheap to fix, and fixing it removes this consequence of the gap regardless of what happens to VAL-H2.

## Remediation options

1. Make the assignment a maximum: `self.next_sequence = self.next_sequence.max(sequence.saturating_add(1));`, and return `None` early when `sequence < self.next_sequence` so a stale sequence cannot select a nonce either. One line each, no behaviour change on a well-ordered chain.
2. Log at `warn` when a `Sign` arrives with a sequence below the recorded one - on an honest chain it never should, so it is a high-signal indicator of either an injected event or a state-machine bug.
3. Independently, bind coordinator events to the coordinator address in `apply_transition` (this is VAL-H2's remediation; noted here only because it removes the trigger).

Tests to add: a `NonceState` unit test asserting `observe` is idempotent-or-monotonic - deliver 1000, then 500, and assert `next_sequence == 1001`, `available` unchanged, and the second call returns `None`.

## Trail

- Reviewer R5: drafted, self-estimate 65% (that the missing bound and the capacity inflation are as described; exploitability is inherited from VAL-H2 and rated lower).

## Critic (C-VAL-B)

Derived from `state/preprocess.rs:174-247` and `state/sign.rs:30-35` before reading the Claim.

### Per-claim verdicts

All five basis rows **Supported**; every quote matches. `observe` really does assign (`self.next_sequence = sequence.saturating_add(1)`, `preprocess.rs:189`) rather than take a maximum, and it really is evaluated before any authorisation of the event (`sign.rs:30-34`, inside the `match` scrutinee). `FROSTCoordinator.sol:536`'s `state.sequence++` really is the strictly-increasing invariant the Rust side declines to assert. No `H` claims.

### Independent check of reachability - I agree with the reviewer's own honesty about it

I looked for an honest-chain path to a non-monotonic `sequence` and found none:

- Log delivery is strictly ordered and duplicate-free: `handle_update` rejects any batch that is not `is_sorted_by(|a, b| (a.block, a.index) < (b.block, b.index))` with `Error::BadUpdate` (`core/state/mod.rs:207-211`), and `is_next_in_range` forces successive batches to advance (`:278-281`).
- A reorg rewinds `State` and the chain together (`core/state/mod.rs:182-189`), so the rolled-back `next_sequence` and the replayed sequences stay consistent.
- Per-group isolation holds: `handle_sign` selects the epoch by `group.id == event.gid` (`sign.rs:33`), and group ids embed the epoch number through `group_context` (`consensus/group.rs:201`, `:268-277`), so two tracked epochs cannot share one `NonceState`.

So the arithmetic defect is real and unconditional, but on the honest chain it is unreachable, and its only route to reachability is F-VAL-060's injectable watched address. The reviewer labels this correctly ("`E2` for the missing bound and the arithmetic; `I` for exploitability, because it inherits VAL-H2's precondition"). That is the right call and I am not going to reward it with a number the precondition cannot support.

### Finding verdict

**Plausible - 40%.** The mechanism is `E2` and the worked arithmetic in the Trigger is correct - I recomputed it: at sequence 8000 (chunk 7, offset 832) `available` is `1024-832 = 192`; one injected `Sign` at 7168 sets `next_sequence = 7169` (chunk 7, offset 1) and `available` becomes 1023. The trigger is inherited wholesale from F-VAL-060, which I have settled at 50% (see my section there), and a finding cannot outrank its own precondition. 40 rather than 45 because the _incremental_ harm over F-VAL-060 is small - anything that can inject a `Sign` log already holds the far stronger primitives F-VAL-060 enumerates.

**Severity: Low (unchanged).** Correct. The consequence is the same silent non-participation as F-VAL-030, reached only through a precondition that already grants worse.

**Remediation note.** The one-line fix (`self.next_sequence = self.next_sequence.max(sequence.saturating_add(1))`) is worth taking on its own merits and independently of F-VAL-060, because it converts a state variable that must be monotonic into one that provably is; pair it with a `debug_assert!` so a future non-monotonic input is loud rather than silent.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **40%**; severity Low unchanged. No PoC directory — the finding's precondition is F-VAL-060, which C-VAL-B settled at 50%, and a finding cannot outrank its own precondition.

### What would be run, and what it would show

The finding's own test, and it is a pure unit test needing no harness at all:

```rust
// crates/validator/src/state/preprocess.rs, a new tests module
#[test]
fn observe_is_monotonic {
    let mut nonces = NonceState { next_sequence: 8000, chunks: [(7, Some(ROOT))].into };
    let before = nonces.available;
    assert!(nonces.observe(7168).is_none);        // a stale, injected sequence
    assert_eq!(nonces.next_sequence, 8001);         // FAILS TODAY: it is 7169
    assert_eq!(nonces.available, before);         // FAILS TODAY: 192 becomes 1023
}
```

I re-derived C-VAL-B's arithmetic independently and it is right: sequence 8000 is chunk 7 offset 832, so `available` is `1024 - 832 = 192`; one `Sign` at 7168 sets `next_sequence = 7169` (chunk 7, offset 1) and `available` becomes 1023. Running that test is `E1` for the mechanism. It would **not** raise the certainty, because the certainty here is bounded by the precondition and not by the mechanism — the same point C-VAL-B makes, and it is worth restating so a green test is not mistaken for a settled finding.

Note the test as written needs `NonceState`'s fields and `available` reachable, which means it must live in `crate::state` (fields are private to that module) and `available` must become `pub(super)` — it is a plain private `fn` today (`state/preprocess.rs:235`). Say that in the ticket; it is the only reason this test does not already exist.

### Remediation check

**Option 1 is sound and is two one-line changes, and I would take it independently of F-VAL-060.** `self.next_sequence = self.next_sequence.max(sequence.saturating_add(1))` plus an early `if sequence < self.next_sequence { return None; }`. The argument for doing it regardless of the precondition is the one C-VAL-B gives and it is the right one: it converts a state variable that _must_ be monotonic into one that provably is, which removes a whole class of future reasoning rather than patching one path. Add the `debug_assert!` C-VAL-B suggests so a non-monotonic input is loud in tests and silent in production.

One thing the option does not say: the early return must come **before** the `split_off`, or a stale sequence still prunes chunks it should not. Written in the order the option gives (assignment first, then return) it would be wrong; written as "return early, then assign" it is right. That ordering is the entire fix and should be explicit in the ticket.

**Option 2 (`warn!` on a below-watermark sequence) is sound and cheap**, and it is the only part that would have surfaced this condition in a running system. On an honest chain it never fires.

**Option 3 (bind coordinator events to the coordinator address) is F-VAL-060's remediation and is correctly cross-referenced rather than duplicated.** It removes the trigger; option 1 removes the mechanism. Both are worth having, and neither substitutes for the other — which is the right way for this finding to be reported alongside its precondition.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty and severity unchanged. Merge commit `a7f3915`, which merges `origin/main` and the Certora FROST audit fixes I-01..I-09. `crates/validator` is untouched by the merge, so this finding's mechanism is byte-identical.

The merge shifts `contracts/src/FROSTCoordinator.sol` by two documentation-only hunks (`9e41b49`: NatSpec on the `SignShared` event and on `signShare`). Every function this file quotes is byte-identical — only its address moved. Corrected citations:

| Old | New |
| --- | --- |
| `FROSTCoordinator.sol:530-542` (Location, basis row 5, excerpt 5) | **`:536-548`** |
| `FROSTCoordinator.sol:536` (`state.sequence++`, the strictly-increasing invariant) | **`:542`** |

## In-flight impact (AS-PRUNE)

**Pertains to unmerged branches, not to `main`.** Assessed against the Scheduled Secret Pruning stack (`origin/prune/end`, PRs #906–#913), built from a `git archive` extraction; no branch was merged or checked out. **Effect: unchanged.** `state/preprocess.rs::observe` is untouched.

## Reconciliation (run 2)

**Final combined status: Low, 40 %, Plausible — run-2 miss, still valid at `fe9e84c`.** `state/preprocess.rs:194` still assigns `next_sequence = sequence.saturating_add(1)` unconditionally. `F2-VAL-061` scenario (b) exercises only the forward jump. See `state/run2/reconciliation/validator.md` (Section 2).
