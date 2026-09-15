# F-CORE-010 The `-32001` recovery commits the block watcher's rewind before the event watcher accepts it, and the hash agreement between them is an unenforced invariant whose violation is a permanent, unrecoverable loop

| Field | Value |
| --- | --- |
| Status | Draft (Critic-promoted) |
| Crate and module | core, `index/mod.rs` and `index/blocks.rs` |
| Location | `crates/core/src/index/mod.rs:106-130` (specifically `113-123`) and `crates/core/src/index/blocks.rs:483-535` (specifically `486-502`, `522-532`); related `crates/core/src/index/events.rs:262-273` |
| Severity | Low / Low |
| Certainty | 45% |
| Assumptions involved | A4, A5 |
| Tags | reorg, crash-consistency, dos |

## Claim

`Watcher::next_logs` drives a two-component recovery whose steps are ordered so that the irreversible one happens first. `BlockWatcher::revalidate_last_block` decides an invalidation and then, before returning, **commits all of it**: it truncates `recent`, rewinds `pending`, clears `queue` and pushes an `Uncle`. Only afterwards does `next_logs` call `self.events.on_block_invalidated(invalidated.hash)?`, which _validates_ the decision by comparing the returned hash against the block the event watcher is actually fetching, and returns `Error::UnexpectedBlockInvalidation` if they differ.

If they ever differ, the `?` propagates the error to the driver **after** the block watcher has already rewound, and the failure is permanent rather than transient:

- `on_block_invalidated`'s mismatch arm returns `Err` _without_ changing `self.step` (`events.rs:271`), so the event watcher stays in `Step::Block { block_hash: h }` for the same block.
- The driver retries (`driver.rs:216-223`). `events.next` re-issues the same by-hash log query and gets the same `-32001`.
- `next_logs` calls `revalidate_last_block` again — but now `queue.front` is the `Uncle` that the first pass pushed, so the guard `Some(_) => return Ok(None)` at `blocks.rs:488` fires unconditionally. `Ok(None)` means "still canonical", so `next_logs` re-raises the original error (`index/mod.rs:124-125`).
- Every subsequent iteration takes the same path. The queued `Uncle` is never delivered, because `Watcher::next` only reaches `blocks.next` when `next_logs` returns `Ok(None)` (`index/mod.rs:85-96`), and it never does. The service spins at 10 RPC calls per second forever, with `/health` answering `OK` (`observability/metrics.rs:9-10`).

**I could not construct a reachable mismatch, and neither could R1** (see below). The finding is therefore about the _shape_ of the code, not a live bug: an invariant that a permanent, silent deadlock depends on is nowhere asserted, nowhere tested, and nowhere documented as an invariant, and the two operations that must agree are separated by a component boundary and sequenced commit-then-validate.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The rewind is committed inside `revalidate_last_block`, before any cross-check. | E2 | `crates/core/src/index/blocks.rs:521-534` | <pre>let timestamp = last.timestamp;<br>self.recent.truncate(last_index);<br>self.pending = PendingBlock {<br> number: invalidated.number,<br> timestamp_ms: timestamp * 1000,<br>};<br><br>// Clear the queue and insert the uncle update.<br>self.queue.clear;<br>self.queue.push_back(BlockUpdate::Uncle {<br> number: invalidated.number,<br>});<br><br>Ok(Some(invalidated))</pre> |
| 2 | The cross-check happens afterwards and its failure propagates rather than resynchronising. | E2 | `crates/core/src/index/mod.rs:113-123` | <pre>match self.blocks.revalidate_last_block.await? {<br> // It was uncled; tell the event watcher to move on.<br> Some(invalidated) => {<br> tracing::debug!(<br> number = invalidated.number,<br> hash = %invalidated.hash,<br> "logs unavailable for uncled block, skipping"<br> );<br> self.events.on_block_invalidated(invalidated.hash)?;<br> Ok(None)<br> }</pre> |
| 3 | The mismatch arm leaves the event watcher's step untouched, so the next attempt repeats identically. | E2 | `crates/core/src/index/events.rs:262-273` | <pre>pub fn on_block_invalidated(&mut self, block_hash: B256) -> Result<, Error> {<br> match self.step {<br> Step::Block {<br> block_hash: current,<br> ..<br> } if current == block_hash => {<br> self.step = Step::Idle;<br> Ok()<br> }<br> _ => Err(Error::UnexpectedBlockInvalidation),<br> }<br>}</pre> |
| 4 | After the first pass, the queue front is an `Uncle`, which makes every later revalidation a no-op. | E2 | `crates/core/src/index/blocks.rs:486-490` | <pre>let next_number = match self.queue.front {<br> Some(BlockUpdate::New { number, .. }) => Some(*number),<br> Some(_) => return Ok(None),<br> None => None,<br>};</pre> |
| 5 | `Ok(None)` re-raises the original error, and the driver retries it forever. | E2 | `crates/core/src/index/mod.rs:124-125` and `crates/core/src/driver.rs:211-223` | <pre>// It is still canonical; the logs just are not available yet.<br>None => Err(err.into),</pre> |
| 6 | The queued `Uncle` can only be delivered through a path this loop never reaches. | E2 | `crates/core/src/index/mod.rs:85-96` | <pre>if let Some(events) = self.next_logs.await? {<br> …<br>} else {<br> let update = self.blocks.next.await?;<br> self.events.on_block_update(update.clone)?;</pre> |
| 7 | Nothing asserts, tests or documents the hash agreement. | E2 (absence) | `crates/core/src/index/blocks.rs:483-535`, `crates/core/src/index/mod.rs:106-130` | Neither function contains a `debug_assert!`, and `grep -n "UnexpectedBlockInvalidation" crates/` matches only the variant's declaration (`events.rs:130`) and its single construction site (`events.rs:271`) — no test constructs or asserts it. |

## Trigger

**None identified.** The mismatch requires `revalidate_last_block`'s `rposition` selection to pick a header other than the one the event watcher is fetching. I re-derived the queue and `recent` shapes independently of R1 and the selection is correct in every one of them:

- **Live path, empty queue** (`next_number = None`): `rposition(|_| true)` returns the back of `recent`, which is the block `next` pushed at `blocks.rs:447` immediately after emitting it as `New`. Match.
- **Draining the init queue** (`queue.front == New { M }`): the queued `New` updates are strictly increasing and drained in order, so the in-flight block is `M - 1` and `rposition(|b| b.number < M)` selects exactly it. Match.
- **Resume with an `Uncle` at the front, or a `Warp` at the front**: the `Some(_) => Ok(None)` guard fires before anything is touched. No invalidation.
- **During a warp**: `queue.front` is the first queued `New { safe + 1 }` and every header in `recent` has a number `>= safe + 1`, so `rposition` finds nothing and returns `Ok(None)`. No invalidation.
- **`start_block` above `safe`, so `recent` holds headers below the first emitted `New`**: the `next_number` filter still selects the highest header below the queue front, which is the block in flight. Match.
- **`max_reorg_depth = 0`**, where `recent` is permanently empty: `rposition` yields `None` and the function returns `Ok(None)` — that is F-CORE-005, a different loop, not this one.
- **Cancellation**: there is no `await` between `revalidate_last_block` returning and `on_block_invalidated` being called, so the driver's `select!` cannot split the pair.

So the finding rests on the argument that a case analysis over queue shapes is the wrong thing for a permanent deadlock to depend on — not on a demonstrated input.

## Considered and rejected

- **"R1 already rejected this."** R1 rejected it as _unreachable_ (coverage log §6, item 2) and recorded it as observation O-1, explicitly asking the Critic to re-derive it because "the consequence is severe and the argument is a case analysis, not an invariant enforced in code". I re-derived it from the code before reading R1's note and reached the same reachability conclusion by an independent route, adding the warp and `start_block > safe` shapes R1 did not enumerate. The promotion is not a disagreement about reachability; it is the position that a defect whose only defence is an unwritten invariant belongs in the record as a Low finding rather than an observation, because the next edit to either component is what breaks it.
- **"The error is transient, so the driver's retry fixes it."** It does not. Basis 3, 4 and 6 make the second and every later attempt take a strictly shorter path than the first, so the loop cannot make progress. This is the one error class in the watcher where a retry is provably useless — which is also what distinguishes it from the general "no terminal error state" problem of F-CORE-004.
- **"`Watcher::next`'s `UnexpectedBlockUpdate` sibling has the same shape."** It does not. R1's rejected hypothesis 1 covers that case and I confirm the rejection: `on_block_update` is called only on the branch where `next_logs` returned `Ok(None)`, which only `Step::Idle` produces (`events.rs:282-283`), and `on_block_update` errors only when not idle (`events.rs:230-232`). There is no await between `blocks.next` resolving and `on_block_update`, so nothing can intervene. That path is genuinely safe; this one is safe only by case analysis.
- **Not a false positive because** the ordering (commit, then validate) and the non-progressing retry are both quoted from this checkout, and basis 7 records that nothing anywhere asserts the property they depend on.

## Remediation options

1. **Make the recovery tolerate the mismatch instead of propagating it.** At `index/mod.rs:121`, treat `Err(UnexpectedBlockInvalidation)` as "the block watcher has already rewound; return `Ok(None)` and let `blocks.next` deliver the queued `Uncle`" rather than as a fatal error. This is R1's own suggestion and is the smallest change: it converts an unrecoverable loop into a resynchronisation, and costs nothing when the invariant holds.
2. **Assert the invariant where it is created.** Add a `debug_assert_eq!` (or a `tracing::error!` in release) inside `revalidate_last_block` comparing the selected header against the block the event watcher reports as in flight. Requires threading one accessor across the component boundary, but turns a silent production deadlock into a test failure.
3. **Reorder to validate before committing.** Have `revalidate_last_block` return the candidate without mutating, let the caller confirm it against the event watcher, and apply the rewind only then. Cleanest, largest change, and it also removes the "queue front is now an `Uncle`" trap in basis 4.
4. Independently: give the driver an escalation path for errors that repeat unchanged on the same step, so _any_ non-progressing loop becomes visible. That overlaps F-CORE-004's remediation and would bound this class as a side effect.

Tests to add: a `Watcher`-level test that drives `Step::Block` for block `h`, forces `revalidate_last_block` to return an `InvalidatedBlock` for a different hash, and asserts the watcher recovers (delivers the `Uncle`) rather than looping. No such test exists — `grep -n "UnexpectedBlockInvalidation" crates/` matches only the declaration and the single construction site. No code is committed.

## Trail

- Critic C-CORE-A: **drafted by the Critic** from R1's observation O-1 and rejected hypothesis 2, which R1 escalated for independent re-derivation. Mechanism `E2` from the quoted code; trigger **none identified** after enumerating seven queue/`recent` shapes. Self-assessed **Plausible, 45%**, Low — filed rather than left as an observation because the mechanism is verified and the certainty rubric puts "mechanism verified, trigger unproven" at 40-69, not below the bar. If the team's view is that an unenforced-but-currently-true invariant is not worth a finding, this should be closed as Rejected with that reason recorded, rather than dropped.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, which is also the smallest change.**

Option 1 (treat `Err(UnexpectedBlockInvalidation)` at `index/mod.rs:121` as "the block watcher has already rewound; return `Ok(None)` and let the queued `Uncle` be delivered") is sound: it converts an unrecoverable loop into a resynchronisation and costs nothing when the invariant holds. That is the right shape for an invariant violation whose only current consequence is a permanent stall.

Option 2 (`debug_assert_eq!` / release-mode `error!` at the point the invariant is created) is sound and complementary — option 1 makes the violation survivable, option 2 makes it visible. Take both; option 1 alone would hide a genuine bug behind a successful recovery.

Option 3 (validate before committing the rewind) is the cleanest design and also removes the "queue front is now an `Uncle`" trap, but it is the largest change and is not justified by a 45% finding on its own. Reasonable to defer.

Option 4 (a driver escalation path for errors that repeat unchanged on the same step) is the same change as F-CORE-004 option 3 and F-CORE-034 option 2, and would bound this whole class as a side effect. That is the strongest argument for prioritising the escalation path over any individual loop fix.

No option touches the state machine or effects; the `core::state` contract is unaffected.
