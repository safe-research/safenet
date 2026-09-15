# F-VAL-039 The nonce top-up threshold gives ~100 sequences of headroom against a permissionless, group-wide sequence counter, so an attacker can force every validator into an unlinked chunk for the length of one `preprocess` round trip

| Field | Value |
| --- | --- |
| Status | QA-done (drafted by Critic C-VAL-B) |
| Crate and module | validator, state/preprocess.rs (with service/action.rs, state/sign.rs) |
| Location | crates/validator/src/state/preprocess.rs:15-17, 85-103, 234-247 (related: crates/validator/src/service/action.rs:237-255, crates/validator/src/state/sign.rs:30-35, contracts/src/FROSTCoordinator.sol:530-542, contracts/src/libraries/FROSTNonceCommitmentSet.sol:91-105) |
| Severity | C-VAL-B: High |
| Certainty | 58% (Critic C-VAL-B; QA may raise) |
| Assumptions involved | A2, A10 |
| Tags | dos, input-validation |

## Claim

R5's coverage log rejects the VAL-H6 sub-claim that an attacker can "drain 1024-nonce chunks faster than the generator can replace them", on the ground that `handle_nonce_topup` triggers at `available < 100` and one reservation restores 1024, so "exhaustion needs sustained ~1000 tx/chunk" and the matter is "a cost-of-attack question, not a defect". That cost model measures the wrong quantity, and the conclusion does not follow from it.

The attacker does not have to sustain anything or exhaust a chunk. They have to keep the group's sequence ahead of the validators' _linked_ chunk for the duration of one top-up round trip. Three properties of the design make that cheap:

1. **The headroom is a hard-coded 100 sequences.** `const NONCE_TOPUP_THRESHOLD: u64 = 100` (`state/preprocess.rs:17`), and `handle_nonce_topup` returns early whenever `available >= NONCE_TOPUP_THRESHOLD` (`:91-93`). So the replenishment cycle does not even begin until fewer than 100 usable coordinates remain.
2. **Closing the gap requires an onchain round trip, not a local computation.** The reservation is local, but the chunk only becomes usable when a `Preprocess` action is mined and its event is observed and linked (`state/preprocess.rs:55-81`). `Action::Preprocess` is queued with **no expiry** and no priority (`service/action.rs:237-255`), behind whatever else is in the transaction queue. That is at minimum one Gnosis block (~5 s, A10) and realistically several.
3. **The sequence counter is per group, and `observe` is called unconditionally.** `sign` is permissionless — `require(message != bytes32(0))`, `status == FINALIZED`, then `uint64 sequence = state.sequence++` (`contracts/src/FROSTCoordinator.sol:530-541`) — and `handle_sign` evaluates `epoch.nonces.observe(event.sequence)` in the `match` scrutinee, before any session matching and for every validator that tracks the group (`state/sign.rs:30-35`). One attacker transaction therefore consumes one coordinate from **every** validator simultaneously.

So the attack is: burn ~100 sequences inside the top-up window. Each burn is one `sign` call on a contract with no access control and no per-caller limit; on Gnosis at the base fees the handbook quotes (`docs/validator-handbook.md:44`) the whole campaign is single-digit dollars, and it can be repeated every time the validators re-link. While the group's sequence sits in an unlinked chunk, `observe` returns `None` for every validator, and — via F-VAL-032 — the next `Sign` for a message they _are_ tracking makes them drop that signing session permanently rather than skip it.

This finding is the _headroom_ defect: a replenishment threshold chosen without reference to the fact that the resource it guards is drained by an unauthenticated, group-wide counter. F-VAL-032 is the _consequence_ defect (the dropped session) and F-VAL-030 is the _bookkeeping_ defect (a phantom reservation counted as capacity). All three have to hold for the worst outcome; each is separately fixable.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The replenishment threshold is a hard-coded 100 and gates the whole top-up path | E2 | crates/validator/src/state/preprocess.rs:15-17, 91-93 | `/// The remaining canonical nonce capacity below which another chunk is`<br>`/// requested.`<br>`const NONCE_TOPUP_THRESHOLD: u64 = 100;` … `        if epoch.nonces.available >= NONCE_TOPUP_THRESHOLD {`<br>`            return (state, Vec::new);`<br>`        }` |
| 2 | A reserved chunk is not usable until the onchain `Preprocess` event links a root; until then `observe` yields `None` | E2 | crates/validator/src/state/preprocess.rs:79, 180-193 | `        epoch.nonces.link(event.chunk, event.commitment);` … `        let nonce = self`<br>`            .chunks`<br>`            .get(&chunk)`<br>`            .copied`<br>`            .flatten`<br>`            .map(\|root\| NonceIndex { root, offset });` |
| 3 | The `Preprocess` action carries no expiry and no prioritisation, so the round trip is at the mercy of the queue | E2 | crates/validator/src/service/action.rs:250-255 | `                    gas: 250_000,`<br>`                },`<br>`                // Nonce registration doesn't carry an expiry - we cannot`<br>`                // reliably know for how long it is valuable.`<br>`                None,` |
| 4 | `Coordinator.sign` is permissionless and increments the group-wide sequence on every call | E2 | contracts/src/FROSTCoordinator.sol:530-541 | `    function sign(FROSTGroupId.T gid, bytes32 message) external returns (FROSTSignatureId.T sid) {`<br>`        require(message != bytes32(0), InvalidMessage);`<br>`        Group storage group = $groups[gid];`<br>`        GroupState memory state = group.state;`<br>`        require(state.count > 0, GroupNotInitialized);`<br>`        require(state.status == GroupStatus.FINALIZED, GroupNotReady);`<br>`        uint64 sequence = state.sequence++;` |
| 5 | Every validator tracking the group advances its own `next_sequence` on every `Sign`, before any matching | E2 | crates/validator/src/state/sign.rs:30-35 | `        let nonce = state`<br>`            .epochs`<br>`            .values_mut`<br>`            .find(\|epoch\| epoch.group.id == event.gid)`<br>`            .and_then(\|epoch\| epoch.nonces.observe(event.sequence));`<br>`        match (nonce, state.signing.remove(&event.message)) {` |
| 6 | The contract's own commit records `startOffset` at commit time, so a chunk linked late is only usable from that offset on | E2 | contracts/src/libraries/FROSTNonceCommitmentSet.sol:97-104 | `        (chunk, offset) = _sequence(sequence);`<br>`        uint64 next = commitments.next;`<br>`        if (next > chunk) {`<br>`            chunk = next;`<br>`            offset = 0;`<br>`        }`<br>`        commitments.next = chunk + 1;`<br>`        commitments.chunks[chunk] = _root(commitment, offset);` |
| 7 | Only the active epoch is ever topped up, so an older participating epoch has no replenishment at all | E2 | crates/validator/src/state/preprocess.rs:86-89 | `        let active_epoch = state.active_epoch;`<br>`        let Some(epoch) = state.epochs.get_mut(&active_epoch) else {`<br>`            return (state, Vec::new);`<br>`        };` |

## Trigger

1. Observe the group's current sequence with `Coordinator.groupParameters`/the public `Sign` stream; wait until the offset within the linked chunk is below 924, i.e. `available >= 100` and no top-up is pending.
2. Submit `sign(gid, <any non-zero message>)` enough times to push `available` below 100 and then past the end of the linked chunk — at most 1024 calls from a cold start, and typically ~100 if timed near a chunk boundary. Nothing rate-limits the caller and nothing binds the message to a proposal.
3. The validators' next `NewBlock` emits `Effect::NonceTree` and, on success, an unexpiring `Action::Preprocess`. Until that transaction is mined and its `Preprocess` event observed, every validator's `observe` returns `None` for the current sequence (basis 2).
4. Inside that window submit `sign(gid, m)` for a message `m` the validators are tracking — a live `TransactionProposed` packet, or the `epoch_rollover_hash` of an in-flight rollover, both of which are publicly derivable. Every validator takes `state/sign.rs:106-114` and drops the session; see F-VAL-032 for why the rollover case has no re-creation path.
5. Repeat each time the validators re-link. Because the contract records `startOffset` at commit time (basis 6), a chunk linked while the sequence is deep inside it yields only `1024 - startOffset` usable coordinates, so a sustained campaign forces increasingly frequent `preprocess` transactions at the validators' expense while the attacker pays only for `sign` calls.

## Considered and rejected

- **"The generator restores 1024 per reservation, so the attacker must sustain ~1000 tx per chunk"** (R5's stated reason for rejecting this). The 1024 is restored _locally_; what the validator can actually use is bounded by the contract's `startOffset` at the moment its `preprocess` executes (basis 6), and the window that matters is the round trip, not the chunk. The attacker needs to win ~100 sequences over a few blocks, not 1000 over a chunk.
- **"This is F-VAL-032."** No. F-VAL-032 is the missing re-insert in one `match` arm and is fixed by one line there. This is the sizing of `NONCE_TOPUP_THRESHOLD` against an adversarial, permissionless counter, and is fixed in `state/preprocess.rs` — for example by scaling the threshold to the observed sequence rate, or by keeping a second linked chunk in reserve at all times so there is never a window with no linked chunk. Both remain worth doing if the other is fixed.
- **"This is F-VAL-030."** No. F-VAL-030 is about a reservation that was _made_ and never filled. This is about the window between a correct reservation and its onchain link, which exists even when every effect succeeds.
- **"A2 does not grant this."** It does: `sign` requires no group membership at all, so the attacker need not be one of the <1/3 dishonest validators — any funded account suffices.
- **Griefing cost.** Not rejected but quantified: ~100 `sign` calls, each a `require` pair plus one `SSTORE` and one event. This is cents-to-dollars on Gnosis and is not a meaningful deterrent for an attacker whose payoff is blocking a Safe transaction attestation or an epoch rollover.

## Remediation options

1. **Keep a linked chunk in reserve.** Trigger the top-up when the _last linked_ chunk is the current one (rather than at a fixed 100 remaining), so there is normally a second linked chunk available and no window in which `observe` can fail. Costs one extra `preprocess` transaction of lead time; removes the window entirely rather than widening it.
2. **Make the threshold a function of observed demand** — for example `max(100, k * sequences observed in the last N blocks)` — so a burn campaign accelerates replenishment instead of outrunning it. Cheap, but still leaves a window if the attacker bursts.
3. **Give `Action::Preprocess` an expiry and a priority.** It is the one action whose latency directly determines this window, and it is currently the only action queued with `None` (`service/action.rs:252-254`) alongside `SetValidatorStaker`. See F-VAL-065.
4. **Top up every participating epoch, not only the active one** (basis 7), so a trailing epoch's ceremonies are not silently unserviceable. R5 recorded this as observation 5 and judged it probably intended; it becomes load-bearing under a burn campaign that straddles a rollover.

Tests to add: a `NonceState` unit test asserting that `available` never reports usable capacity for a chunk with no linked root (this also pins F-VAL-030), and a simulation test advancing `observe` past the end of the linked chunk and asserting the validator still resolves a nonce.

## Trail

- Critic C-VAL-B: drafted. Promoted from `../state/coverage-logs.md#r5`'s rejected VAL-H6 sub-claim ("drains 1024-nonce chunks faster than the generator can replace them" — REJECTED as "a cost-of-attack question, not a defect"). The citation R5 gives is accurate but measures chunk exhaustion rather than the top-up round trip; re-derived from `preprocess.rs`, `action.rs`, `sign.rs` and the two Solidity libraries. Self-estimate 58%: mechanism `E2` throughout, trigger `E2` for steps 1-4, held below the Confirmed band because the burn rate needed to beat one `preprocess` round trip depends on queue latency and block inclusion, neither measurable in a read-only run (`state/baseline.md` §2).

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **58%**; severity High unchanged. No dedicated PoC directory, but the harness this finding needs already exists: the phantom-chunk harness at [`poc/F-VAL-030-032-061/nonce_state.rs`](../poc/F-VAL-030-032-061/nonce_state.rs) builds exactly the `NonceState` values required, and [`a_permissionless_sign_at_an_unlinked_sequence_grieves_a_healthy_validator`](../poc/F-VAL-030-032-061/nonce_state.rs) is the single-shot version of this finding's attack: it shows one attacker-chosen sequence in an unlinked chunk destroying a healthy validator's session. This finding is the _campaign_ version — the claim that the 100-sequence headroom lets an attacker reach that state deliberately rather than opportunistically — and that is the part still needing a test.

### What would be run, and what it would show

Extend `nonce_state.rs` with a loop that models the burn campaign against the real transition:

1. start with `NonceState { next_sequence: 0, chunks: { 0 => Some(root) } }` — a healthy validator with one linked chunk;
2. deliver `Coordinator::Sign` events at sequences `0, 1, 2, …`, applying `Message::NewBlock` every `k` sequences to let `handle_nonce_topup` run;
3. assert the block at which `available` first drops below `NONCE_TOPUP_THRESHOLD` and a `NonceTree` is emitted — that is sequence 924 — and then assert that every sequence from 1024 onward resolves to `None` until a `Preprocess` event links chunk 1.

The measurable output is the **window in sequences** between the top-up firing and the `Preprocess` landing, which is what the whole finding is about. It is `E1` for the mechanism; the _duration_ of that window in wall time is a chain-latency question the test cannot answer, and that is why 58% is the right band rather than the top of `E2`.

### Remediation check

**Option 1 (keep a linked chunk in reserve — trigger on "the last linked chunk is the current one") is sound and is the fix.** It removes the window rather than widening it, which is the correct framing: the current 100-sequence threshold is a _quantity_ answer to a _latency_ problem, and no value of the constant makes it right. The cost the option names — one extra `preprocess` of lead time — is the real cost, and it is small. Note it composes with F-VAL-030 option 2 (exclude `None` reservations from `available`): with both, the trigger becomes "there is no linked chunk beyond the current one", which is a single readable predicate over `chunks`.

**Option 2 (make the threshold a function of observed demand) is sound and I would not take it.** The option concedes it "still leaves a window if the attacker bursts", and it introduces a demand estimator into snapshotted consensus state — new state, new serialisation, a new parameter to get wrong — to achieve less than option 1 does with a predicate. Option 1 is both simpler and strictly stronger.

**Option 3 (give `Action::Preprocess` an expiry and a priority) is sound and is the necessary companion to option 1.** Option 1 shortens the window on the assumption that the `preprocess` transaction lands promptly; that assumption is exactly what F-VAL-065 shows is unwarranted, since `Action::Preprocess` is queued with `expires_at: None` (`service/action.rs:252-254`) and a reverted transaction is recorded as executed and never retried (`crates/core/src/tx/storage.rs:222-235`). If only one of options 1 and 3 is taken, option 1 alone leaves the window open whenever the transaction stalls. Schedule them together.

**Option 4 (top up every participating epoch, not only the active one) is sound and is more load-bearing than the option suggests.** It is not only about a burn campaign straddling a rollover: **F-VAL-061**'s fresh-epoch variant shows that an epoch created by `finalize_key_gen` (`state/keygen.rs:1307-1320`) gets exactly one `Effect::NonceTree` and is never topped up until it becomes active, because `handle_nonce_topup` reads `state.active_epoch` only (`state/preprocess.rs:86-89`). So option 4 also repairs a lost chunk-0 reservation for a not-yet-active epoch. That makes it the highest-value item in this list after option 1, and it should be cross-referenced to F-VAL-061 rather than filed as a burn-campaign hardening.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty and severity unchanged. Merge commit `a7f3915`, which merges `origin/main` and the Certora FROST audit fixes I-01..I-09. `crates/validator` is untouched by the merge, so this finding's mechanism is byte-identical.

The merge shifts `contracts/src/FROSTCoordinator.sol` by two documentation-only hunks (`9e41b49`: NatSpec on the `SignShared` event and on `signShare`). Every function this file quotes is byte-identical — only its address moved. Corrected citations:

| Old | New |
| --- | --- |
| `FROSTCoordinator.sol:530-542` (Location) | **`:536-548`** |
| `FROSTCoordinator.sol:530-541` (basis row 4, `sign` is permissionless) | **`:536-547`** |
| `FROSTNonceCommitmentSet.sol:91-105` | unchanged — the file was not touched |
