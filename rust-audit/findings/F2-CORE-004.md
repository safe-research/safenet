# F2-CORE-004 Event filtering and decoding are address-agnostic: core cannot bind an event set to its emitting contract, and the validator dispatches coordinator/consensus events without checking `EventLog.address`

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, index/events.rs |
| Location | crates/core/src/index/events.rs:402-411, 458-465, 491-519, 536-594 (related: crates/validator/src/main.rs:57, crates/validator/src/state/mod.rs:415-460) |
| Severity | Low (core design) / Low |
| Certainty | 70% (Critic C2-CORE-A; reviewer self-estimate 70%) |
| Assumptions involved | A1, A2 |
| Tags | input-validation |

Audited commit: `3ec8bc5`.

## Claim

`EventWatcher` takes a flat `Vec<Address>` and a flat topic list (the union of every `*Events` enum's selectors) and issues `eth_getLogs` with `address(all) x topic0(all)`; the client-filtered path applies the same two independent membership tests; `decode_and_sort` decodes by topics and data only and merely attaches the emitter as `EventLog.address`. Any watched address can therefore deliver any watched event type to the consumer's transition function, and core offers no way to express "topic set T_i is only valid from address A_i".

The sentinel's watched set is two protocol contracts, so the property is harmless there. The validator appends operator-configured oracle contracts to the same address list and dispatches every `Coordinator::*` and `Consensus::*` event without consulting `log.address`; only `OracleResult` receives the address. An oracle contract is third-party code (the operator trusts it for _results_, and it may be upgradeable); if it emits a log whose `topic0` and layout match `Coordinator.Sign`, `KeyGenComplained`, `SignRevealedNonces`, etc., the validator's state machine treats it as a coordinator message. The root cause is in core; the reachable impact (which handlers, what state they touch) is R6's to assess.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | One flat filter over all addresses and all topics | E2 | crates/core/src/index/events.rs:405-409 | `let filter = blocks` / `.into_filter()` / `.address(self.addresses.clone())` / `.event_signature(self.topics.clone());` / `let logs = self.provider.get_logs(&filter).await?;` |
| 2 | Client-side filtering is two independent membership tests | E2 | crates/core/src/index/events.rs:458-465 | `.filter(\|log\| {` / `self.addresses.contains(&log.address())` / `&& log` / `.topic0()` / `.is_some_and(\|topic\| self.topics.contains(topic))` / `})` |
| 3 | Decoding ignores the address and only attaches it | E2 | crates/core/src/index/events.rs:498-505 | `E::decode_log(log.topics(), &log.data().data)` / `.and_then(\|data\| {` / `Some(EventLog {` / `block: log.block_number?,` / `index: log.log_index?,` / `address: log.inner.address,` / `data,` / `})` |
| 4 | The macro decodes by trying every enum in order, address-free | E2 | crates/core/src/index/events.rs:577-591 | `fn decode_log(` / `topics: &[::alloy::primitives::B256],` / `data: &[u8],` / `) -> ::std::option::Option<Self> {` / `$(` / `if let ::std::result::Result::Ok(event) =` / `<$events as ::alloy::sol_types::SolEventInterface>::decode_raw_log(` |
| 5 | The validator watches operator-configured oracle addresses with the same set | E2 | crates/validator/src/main.rs:57 | `watched.extend(config.validator.oracles.iter().copied());` |
| 6 | The validator passes `log.address` only to the oracle handler | E2 | crates/validator/src/state/mod.rs:437-439, 458-460 | `Event::Coordinator(Coordinator::CoordinatorEvents::Sign(event)) => {` / `self.handle_sign(state, log.block, &event)` ... `Event::Oracle(Oracle::OracleEvents::OracleResult(event)) => {` / `self.handle_oracle_result(state, log.block, log.address, &event)` |
| 7 | Existing test shows the address is used only as a set-membership filter | E1 | crates/core/src/index/events.rs:885-957 (run this session, passes) | `// A single query returns every log in the block; the watcher keeps only` / `// those from a watched address with a watched event.` |
| 8 | The sentinel's event set is two protocol contracts | E2 | crates/sentinel/src/bindings.rs:165-171 | `// The event set consumed by the Watcher and StateMachine: all events from` ... `Oracle(oracle::SentinelOracle::SentinelOracleEvents),` / `Consensus(consensus::Consensus::ConsensusEvents),` |

## Trigger

Validator configuration lists oracle `O`. `O`'s code emits a log with `topic0 == Coordinator::Sign::SIGNATURE_HASH` and an ABI-compatible body. The watcher's filter matches (`O` is a watched address, the topic is watched), `decode_log` yields `Event::Coordinator(CoordinatorEvents::Sign(..))`, and `handle_sign` runs with an attacker-chosen session. Whether that burns nonces, opens sessions or times out honest ones is R6's trace (not performed here).

## Considered and rejected

- _Operator configuration is trusted (A1)._ The operator's _config_ is trusted; the oracle contract's _code_ is a third party's and A1 does not extend to it. The validator authors evidently expected an address check where it mattered (`OracleResult`), which core does not help with elsewhere.
- _Same-signature events across the protocol contracts._ Checked the `event` declarations in both `bindings.rs` files: no event name is shared between contracts watched by the same service, so mis-attribution among protocol contracts does not occur today.
- _The sentinel is affected._ Its watched addresses are protocol contracts only (per the bindings; the sentinel's address list in `main.rs` was not re-read, R7).

## Remediation options

1. Make the watcher group-aware: `EventWatcher::new(provider, config, Vec<(Address, Vec<B256>)>)`, keep the single union query, and post-filter every log by `(address, topic0)` pair before decoding. No extra RPC; the change is confined to core.
2. Pass the address into `Events::decode_log` and let `watcher_events!` bind each variant to an address (or a predicate) so mis-attributed logs decode to `None` — and then treat `None` as "skip" rather than `DecodeLog` (see F2-CORE-005).
3. Minimal: services check `log.address` against the expected contract in each dispatch arm.

Tests to add: a log from a watched address carrying another group's topic is dropped; the validator-level test belongs to R6.

## Trail

- Reviewer R1: drafted, self-estimate 70% for the core mechanism; validator impact deferred to R6 (seam noted in `state/run2/agents/R1.md`). Confirms lead CORE-H4 at the core level.

## Critic (C2-CORE-A)

Independent read of `crates/core/src/index/events.rs:402-411, 458-465, 491-519, 536-594`: one `address(Vec)` x `event_signature(Vec)` filter; client-side filtering is two independent `contains` tests; `decode_and_sort` calls `E::decode_log(topics, data)` and only attaches `log.inner.address`; the macro tries every `*Events` enum in order. Core offers no `(address, topic set)` binding. Callers checked as the brief asked: `crates/validator/src/main.rs:56-57` watches `[consensus, coordinator]` extended by `config.validator.oracles`; `crates/validator/src/state/mod.rs:415-460` passes `log.address` only to `handle_oracle_result` (`crates/validator/src/state/sign.rs:172-189`, `if expected == oracle && event.approved`); `crates/sentinel/src/main.rs:75-79` watches `vec![config.oracle, config.consensus]`, both protocol contracts (`contracts/src/SentinelOracle.sol`, `contracts/src/Consensus.sol`). This settles R1's rejected item 20, which R1 had not re-read: the sentinel is unaffected.

Per-claim verdicts: 1-8 **Supported** (claim 7's test re-run; claim 8's comment is at `bindings.rs:165-166`, enum at 167-172). Rejected item 9 re-checked: the 17 validator and 11 sentinel event names are pairwise distinct across the contracts each service watches, so no signature collision exists among protocol contracts.

Severity reasoning: the only route is an operator-configured oracle contract emitting a `Coordinator`/`Consensus`-shaped log. The same configuration line already trusts that contract to decide whether the validator attests a transaction (`crates/validator/src/config.rs:66-69`, `sign.rs:179-189`), and a malicious oracle can therefore already have arbitrary proposed Safe transactions attested - which PROMPT §8 rates Critical. Forging coordinator logs crosses no trust boundary the deployment has not already crossed; it widens what a compromised oracle can reach (session and key-generation handlers) without creating a new attacker. Under A1 the operator's choice of oracle is trusted, and nothing in `contracts/src` lets a third party insert an oracle into a validator's list.

Verdict: **Confirmed** (the defect exists as stated and the validator side does not bind addresses). Certainty **70** (E2). Severity **Low / Low** - defence in depth, correctly attributed to core. Remediation 1 is sound and local; remediation 2 must be paired with treating `None` as skip, otherwise F2-CORE-005(a) turns a dropped log into a stall. The per-handler trace of what a forged `Sign` or `KeyGenComplained` would do remains R4/R6 territory and does not change the core severity. A16/A17 not applicable.

### Addendum (C2-CORE-A, after the Coverage Critic's note in `state/run2/coverage.md` §3.6 / §6)

Question put to me: does any Basis row depend on decode failures being errors, given that out-of-range `sol!` enum values decode to `__Invalid` rather than failing? Re-derived from the pinned sources (`Cargo.lock`: `alloy-sol-types` 1.6.0, `alloy-sol-macro-expander` 1.6.0), the decode path for a foreign contract's log that reaches `decode_and_sort` (`crates/core/src/index/events.rs:498`) is:

1. `watcher_events!` calls `<*Events as SolEventInterface>::decode_raw_log(topics, data)` (`events.rs:550-553, 582-585`); the generated interface tries each event's `SolEvent::decode_raw_log` in turn and returns the first `Ok` (`alloy-sol-macro-expander-1.6.0/src/expand/contract.rs:841-852`). The **non-validating** path is used; `decode_raw_log_validate` exists (`alloy-sol-types-1.6.0/src/types/event/mod.rs:202-212`) and is not called.
2. Topics: `TopicList::detokenize` errors only when there are **too few** topics (`iter.next().ok_or_else(length_mismatch)?`, `src/types/event/topic_list.rs:50-53`); **extra topics are silently ignored**. `check_signature` then requires `topics.0 == SIGNATURE_HASH` (`expand/event.rs:104-109`).
3. Data: `abi_decode_data` = `abi_decode_sequence` without validation (`event/mod.rs:169-172, 186-195`): the body must be decodable by layout (length and offsets), nothing more. A `sol!` enum with fewer than 256 variants carries a hidden `__Invalid = u8::MAX` and detokenizes with `try_from(u8).unwrap_or(Self::__Invalid)` (`expand/enum.rs:40-63`), so an out-of-range byte in `Operation` (`crates/validator/src/bindings.rs:35`, `crates/sentinel/src/bindings.rs:77`) or `RequestState` (`crates/sentinel/src/bindings.rs:14`, carried by `DisputeResolved` at line 43) yields a typed event with an `__Invalid` field - **neither an error nor a panic**. `String` fields decode lossily (R7 rejected 1, per the Coverage Critic).

Net: a foreign log needs only the watched `topic0`, at least the expected topic count, and a layout-decodable body to be delivered to the transition function as a fully typed event; it does not need semantically valid contents. Only a topic shortfall or a body that cannot be decoded by layout yields `Err` -> `None` -> `Error::DecodeLog` (that part is core code, `events.rs:498-514`, and stands).

Per-row re-check: rows 1-8 make no claim that a mismatched log fails to decode - row 4 quotes the first-`Ok`-wins loop, and the Trigger already assumes "an ABI-compatible body". **No row is marked `H`.** If anything the finding is understated: the decode path is more permissive than "ABI-compatible" suggests, which strengthens the address-binding remediation (option 1) and further weakens option 2 as stated (decoding to `None` is not how enum or string mismatches manifest). F2-CORE-005(a) is unaffected: its `DecodeLog` route relies on topic-count and data-length mismatches (the crate's ERC-20/ERC-721 test), not on enum ranges. Verdict, certainty and severity unchanged: Confirmed, 70, Low / Low.

## Anchors at fe9e84c (Manager)

Anchors in `crates/sentinel/src/bindings.rs` moved with the `origin/main` merge (`fe9e84c`): line 14 → 15, 77 → 79, 165–171 → 167–173. Content unchanged; mechanism and verdict unaffected (`state/run2/baseline-delta.md` §3).

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-006` (canonical for the core half; `F-VAL-060` for the validator side) — combined Low, 70 (E2).** Run 1 had the same missing `(address, topic set)` binding at Plausible 55; this file's Critic verified both watched sets and the alloy decode permissiveness, which lifts it to Confirmed 70 (`state/run2/reconciliation/core.md` §1).
