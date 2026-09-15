# F-CORE-012 `use_client_filtering`'s bloom-equality completeness check is blind to the loss of any log whose (address, topics) shape another log in the same block repeats — which is the shape of every per-participant ceremony event

| Field | Value |
| --- | --- |
| Status | Draft (Critic-promoted) |
| Crate and module | core, `index/events.rs` and `index/bloom.rs` |
| Location | `crates/core/src/index/events.rs:441-466` (specifically `450`) and `crates/core/src/index/bloom.rs:37-40` (related: `events.rs:471-486`; consumer shape: `crates/validator/src/bindings.rs:191-208`) |
| Severity | Medium / Medium |
| Certainty | 70% |
| Assumptions involved | A4, A6, A10 |
| Tags | input-validation, consensus, dos |

## Claim

`Fetch::ClientFiltered` is the only path in the crate that verifies a node served a _complete_ set of logs. Its test is `bloom::compute_logs_bloom(&logs) != logs_bloom`, i.e. equality between the bloom recomputed over the returned logs and the bloom in the block header.

A block's `logsBloom` is a **bit union**: each log contributes `M(address) | M(topic_0) | … | M(topic_n)` and the results are OR-ed together. The union is therefore idempotent for repeated inputs — two logs with the same emitter and the same topic list contribute exactly the same bits. Consequently the equality test cannot detect the loss of a log whose `(address, topics)` shape is still represented by at least one surviving log in the same block. Dropping five of six identical- shaped logs passes; dropping all six fails.

This is not a corner case for Safenet. Every per-participant `FROSTCoordinator` event indexes only session-scoped fields, so the _n_ validators responding to the same ceremony step in the same block emit logs that are bit-identical from the bloom's point of view:

- `Preprocess(bytes32 indexed gid, address participant, uint64 chunk, bytes32 commitment)` — one indexed field, the group id, shared by every participant.
- `KeyGenCommitted`, `KeyGenSecretShared`, `KeyGenConfirmed`, `KeyGenComplained`, `KeyGenComplaintResponded` — all `bytes32 indexed gid` only.
- `SignRevealedNonces(bytes32 indexed sid, …)` — one indexed field, the session id.
- `SignShared(bytes32 indexed sid, bytes32 indexed selectionRoot, …)` — two indexed fields, both session-scoped.

`participant` is _not_ indexed in any of them, so it never reaches the bloom. On a ~5 s chain (A10) several participants answering the same ceremony step land in the same block routinely — that is the normal case, not the exceptional one. A node that returns a truncated or partial log set for such a block passes the integrity check and the watcher commits the block as fully processed.

The gap is compounded by a second fact: `check_logs_limit` — the guard against a node silently capping a response — is **not called on the `ClientFiltered` path at all**. The one path with a completeness check is the one path with no truncation check, and truncation is precisely the failure mode whose surviving prefix keeps one log of each shape.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The completeness check is a single bloom equality over the returned logs. | E2 | `crates/core/src/index/events.rs:444-456` | <pre>let filter = BlockFilter::Hash(block_hash).into_filter;<br>let logs = self.provider.get_logs(&filter).await?;<br><br>// Verify the node served a complete set of logs for the block by<br>// recomputing the bloom filter over every returned log.<br>if bloom::compute_logs_bloom(&logs) != logs_bloom {<br> tracing::warn!(<br> hash = %block_hash,<br> "incomplete logs served for block, bloom filter mismatch"<br> );<br> return Err(Error::IncompleteLogs { block_hash });<br>}</pre> |
| 2 | The recomputation is a plain fold over the logs' `address` and `topics` — no count, no ordering, no per-log identity. | E2 (call site) / I (library internals, A6) | `crates/core/src/index/bloom.rs:37-40` | <pre>/// Computes the bloom filter for some logs.<br>pub fn compute_logs_bloom(logs: &[Log]) -> Bloom {<br> alloy::primitives::logs_bloom(logs.iter.map(&#124;log&#124; &log.inner))<br>}</pre> |
| 3 | The crate itself documents the bloom as a lossy membership structure whose address and topic tests are independent — i.e. it carries no multiplicity information. | E2 | `crates/core/src/index/bloom.rs:16-22` | <pre>/// Because bloom membership has no false negatives, a `false` result guarantees<br>/// the block holds no matching log and its logs need not be fetched. A `true`<br>/// result is only a maybe, since bloom false positives are possible. Note that<br>/// the address and topic checks are independent: a block where one log supplies<br>/// a watched address and a different log supplies a watched topic still returns<br>/// `true`, matching the conservative behaviour of the reference implementation.</pre> |
| 4 | The truncation guard is never applied on this path. | E2 | `crates/core/src/index/events.rs:441-466` | The `Fetch::ClientFiltered` arm calls `self.provider.get_logs`, the bloom check, then `logs.into_iter.filter(…).collect`. `self.check_logs_limit` appears only in the `SingleQuery` arm (`:410`) and the `MultipleQueries` arm (`:420`). |
| 5 | Every per-participant coordinator event indexes only session-scoped fields, so co-block logs from different participants are bloom-identical. | E2 | `crates/validator/src/bindings.rs:191-208` | <pre>event KeyGenConfirmed(bytes32 indexed gid, address participant, bool confirmed);<br>event KeyGenComplained(bytes32 indexed gid, address plaintiff, address accused, bool compromised);<br>…<br>event Preprocess(bytes32 indexed gid, address participant, uint64 chunk, bytes32 commitment);<br>…<br>event SignRevealedNonces(bytes32 indexed sid, address participant, SignNonces nonces);<br>event SignShared(bytes32 indexed sid, bytes32 indexed selectionRoot, address participant, uint256 z);</pre> |
| 6 | An accepted (short) result is committed as the complete state for the block. | E2 | `crates/core/src/index/events.rs:383-397` and `crates/core/src/state/mod.rs:236` | <pre>self.step = if result.is_ok {<br> Step::Idle</pre><br><pre>self.snapshots.commit(blocks.last, &state).await?;</pre> |
| 7 | A4 admits exactly this class of RPC answer. | I (assumption) | `rust-audit/state/reviewer-brief.md` §3 | "A4 **malicious RPC is OUT of scope**, but stale / rate-limited / incomplete `eth_getLogs` results are IN scope" |

## Trigger

No attacker is required; the trigger is an incomplete `eth_getLogs` response, which A4 admits.

1. A validator runs with `use_client_filtering = true` (the handbook's remedy, `docs/validator-handbook.md:37-40`).
2. Block `N` contains, say, five `Preprocess` logs — one per participant answering the same nonce chunk for the same `gid`, which is the ordinary shape of a preprocessing round on a 5 s chain (A10: nonce chunk 1024, `blocks_per_epoch` 1440). All five have emitter = the coordinator and `topics = [keccak256("Preprocess(bytes32,address,uint64,bytes32)"), gid]`.
3. The node returns only the first two — a response-size cap, a paginating gateway, or a partial index. `compute_logs_bloom` over those two sets exactly the same bits as over all five, because the address and both topics are identical across them and the differing fields (`participant`, `chunk`, `commitment`) live in `data`, which the bloom never sees.
4. `compute_logs_bloom(&logs) == logs_bloom`, so no `IncompleteLogs` is raised. `check_logs_limit` is not reached on this path, so the truncation is not caught there either.
5. The two logs are decoded, `Step::Idle` is set, and `StateMachine::handle_update` commits a snapshot at block `N`. The three missing participants' commitments are lost permanently; the block is never re-fetched.

The same construction works for `KeyGenSecretShared` and `KeyGenConfirmed` during a DKG and for `SignRevealedNonces`/`SignShared` during a signing round — i.e. for exactly the messages whose loss diverges the validator from its group.

## Considered and rejected

- **"The bloom would change because the logs differ."** They differ only in un-indexed fields. The bloom accumulates `address` and `topics` only (basis 2, 3), and `participant` is not indexed in any of the events listed in basis 5. Two `Preprocess` logs for the same `gid` are indistinguishable to the bloom.
- **"`check_logs_limit` catches truncation."** Not here: it is not called on the `ClientFiltered` path (basis 4), and even where it is called it is `None` by default (`events.rs:100`) and tests for a response being _too long_, not too short.
- **"A node that truncates would drop whole shapes too, so the check fires."** Sometimes, but not reliably: a size-capped response is a _prefix_, and a prefix of a block's logs generally retains at least one log of the shapes that appear early. The check's guarantee degrades from "complete" to "at least one log of each distinct (address, topics) shape present", which is not the property the feature is sold on.
- **"This is R1's rejected hypothesis 18, already answered."** R1 recorded (coverage log §6, item 18) that `check_logs_limit` is missing on the `ClientFiltered` path but rejected it because "a truncated response removes bloom bits, so the equality check at `events.rs:450` catches it more strongly than a count threshold would." That premise is false for repeated log shapes, and the repeated shape is the normal case for this system's ceremony events. This finding is the promotion of that dismissal.
- **"A6 makes the bloom claim unverifiable."** Only the `alloy::primitives::logs_bloom` _implementation_ is unverifiable (its source is not on disk — `state/baseline.md` §1), and that is basis 2's `I` component. The property the finding rests on is the _definition_ of a block's `logsBloom` as an OR-fold over each log's address and topics, which the header value must satisfy for the equality to hold at all, and which the crate's own doc comment restates (basis 3). If `logs_bloom` did not have that shape the check would fail on every block, not pass on some.
- **Not a false positive because** the check is exercised by three tests (`events.rs:885`, `:1104`, `:1324`) and none of them puts two same-shape logs in one block, so nothing in the suite would notice.

## Remediation options

1. **Add a count to the completeness check.** Compare the _number_ of logs returned against an expectation the node cannot influence — there is none available from the header, so the practical form is to cross-check the `ClientFiltered` result against a second, independent query (`MultipleQueries` on the same block hash) and raise `IncompleteLogs` on any discrepancy. Tradeoff: doubles the request count for the path that already fetches every log in the block.
2. **Call `check_logs_limit` on the `ClientFiltered` path too** and give `max_logs_per_query` a non-`None` default. This does not close the hole but converts the common truncation case into an error instead of a silent short read. Cheap; should be done regardless.
3. **Fetch by `blockHash` and compare against the block's `transactionCount`/receipts** — the only node-independent completeness evidence available — or fetch `eth_getBlockReceipts` for the block and rebuild the log set from it. Tradeoff: another RPC method, more bandwidth, and provider support varies.
4. **State the guarantee honestly.** Whatever is implemented, the doc comment on `use_client_filtering` (`events.rs:80-84`) should say what the check does and does not detect, so an operator is not told a partial response is impossible.

Tests to add: a `ClientFiltered` test whose block contains two logs with identical address and topics, where the mocked node returns only one, asserting `Error::IncompleteLogs`. It fails today. Also a test asserting `check_logs_limit` is applied on the client-filtered path. No code is committed.

## Trail

- Critic C-CORE-A: **drafted by the Critic** while mining R1's coverage log (`rust-audit/../state/coverage-logs.md#r1` §6, rejected hypothesis 18). R1 identified the missing `check_logs_limit` on this path but dismissed it on the premise that bloom equality is the stronger guard; that premise does not hold for repeated `(address, topics)` shapes, which is the shape of every per-participant `FROSTCoordinator` event. Mechanism `E2` from the cited code plus the definition of a block's `logsBloom`; the `alloy` implementation itself is `I` under A6. Self-assessed **Confirmed, 70%**, Medium. Escalate to High alongside F-CORE-002 if a truncating or paginating provider is confirmed in the intended deployment — both findings then describe the same realised outcome (a block committed as complete while missing ceremony messages) through two independent mechanisms.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 2 now, option 3 as the real fix. Option 1 is weaker than it looks.**

Option 2 (call `check_logs_limit` on the `ClientFiltered` path too, and give `max_logs_per_query` a non-`None` default) is cheap, correct and should be done regardless — it does not close the hole but converts the common truncation case from a silent short read into an error. Note the default is currently `None` (`index/events.rs:99-100`), so on a stock deployment that check is inert on _every_ path, not just this one.

Option 1 (cross-check `ClientFiltered` against an independent `MultipleQueries` on the same block) is sound in principle but weaker than its framing suggests: both queries go to the **same node**, so a node that consistently omits the same log satisfies both. It detects inconsistency, not incompleteness. Worth saying plainly, because "compare two queries" reads like a completeness proof and is not one.

Option 3 (`eth_getBlockReceipts`, or compare against the block's transaction count) is the only node-independent completeness evidence available and is the genuine fix. Its stated costs — another RPC method, more bandwidth, variable provider support — are all real, and the provider-support one is the blocker: an operator on a provider without `eth_getBlockReceipts` would be back to option 2.

Option 4 (state the guarantee honestly in the `use_client_filtering` doc comment) is necessary and should be merged with **F-CORE-002 option 4**, which asks for a doc change to the same four lines (`events.rs:80-84`) about a different blind spot. One doc change, two findings — and it must cover both: the check does not survive the retry budget (F-CORE-002) _and_, while it is running, it cannot see a repeated-shape omission (this finding).
