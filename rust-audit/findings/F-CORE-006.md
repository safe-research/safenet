# F-CORE-006 The event watcher matches on the cross product of watched addresses and watched topics, so any watched address can emit any watched event and the decoded value carries no authority binding

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `index/events.rs` |
| Location | `crates/core/src/index/events.rs:400-469` (specifically `404-411`, `412-419`, `458-465`) and `489-519` (related: `198-222`, `530-594`) |
| Severity | Medium / Low |
| Certainty | 55% |
| Assumptions involved | A2, A1 |
| Tags | input-validation, consensus |

## Claim

`EventWatcher` is constructed with one flat `Vec<Address>` and one flat `Vec<B256>` of topic0 values, and every fetch strategy filters on their _cross product_: a log is accepted if its emitter is any watched address **and** its topic0 is any watched event signature. There is no way to express "this address may emit these events" — not in the API, not in the filter, and not in the decoder. `decode_and_sort` then tries each configured ABI in order and returns the first that decodes, so the emitter plays no part in choosing how a log is interpreted. The emitter survives only as an untyped `EventLog::address` field that consumers must remember to check.

Consequently, any address the operator adds to the watch list gains the ability to inject any event in the watched set. That is safe when every watched address is a protocol contract, but the validator deliberately watches third-party oracle contracts alongside `Consensus` and `FROSTCoordinator` (`crates/validator/src/main.rs:56-57`), and the configuration documents those as trusted for their _results_ only (`crates/validator/src/config.rs:66-69`). The validator's dispatch checks the emitter for exactly one of its fifteen handlers — `handle_oracle_result`, the one that receives `log.address` — and dispatches every `Coordinator::*` and `Consensus::*` event without it (`crates/validator/src/state/mod.rs:414-460`). An oracle contract can therefore drive the validator's key-generation and signing state machines with forged `Sign`, `KeyGenComplained`, `SignRevealedNonces`, `Preprocess` or `TransactionProposed` messages that never touched the real coordinator.

This is filed as a core finding because the mechanism and the missing affordance are in `index/events.rs`: the crate offers no per-address topic set, so a service cannot express the restriction even if it wants to, and the safe default (bind the ABI to its contract) is not available. The consumer-side impact and its severity belong to the validator reviewer (lead VAL-H2).

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The default new-block/warp query is one filter over all addresses and all topic0s — a cross product, with no per-address association. | E2 | `crates/core/src/index/events.rs:404-410` | <pre>Fetch::SingleQuery(blocks) => {<br> let filter = blocks<br> .into_filter<br> .address(self.addresses.clone)<br> .event_signature(self.topics.clone);<br> let logs = self.provider.get_logs(&filter).await?;<br> self.check_logs_limit(logs)?<br>}</pre> |
| 2 | The per-topic fallback keeps the full address list on every query, so it is the same cross product split across requests. | E2 | `crates/core/src/index/events.rs:415-419` | <pre>let filter = blocks<br> .into_filter<br> .address(self.addresses.clone)<br> .event_signature(*topic);<br>let logs = self.provider.get_logs(&filter).await?;<br>self.check_logs_limit(logs)</pre> |
| 3 | Client-side filtering applies the identical cross-product predicate, so enabling `use_client_filtering` does not tighten it. | E2 | `crates/core/src/index/events.rs:458-465` | <pre>logs.into_iter<br> .filter(&#124;log&#124; {<br> self.addresses.contains(&log.address)<br> && log<br> .topic0<br> .is_some_and(&#124;topic&#124; self.topics.contains(topic))<br> })<br> .collect</pre> |
| 4 | The watcher's only address state is a flat list; there is nowhere to record which events an address is authorised to emit. | E2 | `crates/core/src/index/events.rs:198-206` | <pre>/// Watches for the logs of the events `E` emitted by a set of addresses.<br>pub struct EventWatcher<E> {<br> provider: Provider,<br> config: Config,<br> addresses: Vec<Address>,<br> topics: Vec<B256>,<br> step: Step,<br> _events: PhantomData<fn -> E>,<br>}</pre> |
| 5 | Decoding ignores the emitter: the address is copied into the output but never used to select the ABI. | E2 | `crates/core/src/index/events.rs:497-506` | <pre>.map(&#124;log&#124; {<br> E::decode_log(log.topics, &log.data.data)<br> .and_then(&#124;data&#124; {<br> Some(EventLog {<br> block: log.block_number?,<br> index: log.log_index?,<br> address: log.inner.address,<br> data,<br> })<br> })</pre> |
| 6 | The generated decoder tries each contract's ABI in declaration order and returns the first success, so a log from any address decodes into whichever contract's event matches. | E2 | `crates/core/src/index/events.rs:577-591` | <pre>fn decode_log(<br> topics: &[::alloy::primitives::B256],<br> data: &[u8],<br>) -> ::std::option::Option<Self> {<br> $(<br>        if let ::std::result::Result::Ok(event) =<br>            <$events as ::alloy::sol_types::SolEventInterface>::decode_raw_log(<br> topics, data,<br> )<br> {<br> return ::std::option::Option::Some(Self::$variant(event));<br> }<br> )*<br> ::std::option::Option::None<br>}</pre> |
| 7 | The validator adds operator-configured third-party oracle contracts to the same flat address list. | E2 | `crates/validator/src/main.rs:56-57` | <pre>let mut watched = vec![consensus, coordinator];<br>watched.extend(config.validator.oracles.iter.copied);</pre> |
| 8 | Those oracles are declared trusted for their results, not for the protocol event stream. | E2 | `crates/validator/src/config.rs:66-69` | <pre>/// The oracle contracts whose results the validator honors when signing<br>/// oracle transactions.<br>#[serde(default)]<br>pub oracles: BTreeSet<Address>,</pre> |
| 9 | Exactly one validator handler receives the emitting address; every coordinator and consensus handler ignores it. (Consumer side, R6's file; cited to show the affordance is in fact unused.) | E2 | `crates/validator/src/state/mod.rs:440-459` | <pre>Event::Coordinator(Coordinator::CoordinatorEvents::Sign(event)) => {<br> self.handle_sign(state, log.block, &event)<br>}<br>Event::Coordinator(Coordinator::CoordinatorEvents::SignRevealedNonces(event)) => {<br> self.handle_sign_revealed_nonces(state, log.block, &event)<br>}<br>...<br>Event::Oracle(Oracle::OracleEvents::OracleResult(event)) => {<br> self.handle_oracle_result(state, log.block, log.address, &event)<br>}</pre> |

## Trigger

1. A validator is configured with `oracles = ["0xORACLE"]` (a supported, documented option; the sample config shows it at `crates/validator/validator.sample.toml:34-36`), so the watcher's address list is `[consensus, coordinator, 0xORACLE]` and the topic list is the union of the `Consensus`, `FROSTCoordinator` and `Oracle` event selectors (basis 7).
2. `0xORACLE` emits a correctly ABI-encoded `FROSTCoordinator.Sign` log: four topics (`keccak256("Sign(address,bytes32,bytes32,bytes32,uint64)")`, an `initiator`, a `gid` matching the validator's live group, and an arbitrary `message`) and a data payload with a fresh `sid` and the next `sequence`. Under assumption A2 the contents of any participant's on-chain message are attacker-controlled, and `FROSTCoordinator.sign` is permissionless in any case.
3. The filter accepts the log — address ∈ watched, topic0 ∈ watched (basis 1) — and `decode_log` returns `Event::Coordinator(CoordinatorEvents::Sign(..))`, because the first ABI whose selector and arity match wins (basis 6).
4. `Transition::apply_transition` dispatches it to `handle_sign` with no emitter check (basis 9), so the validator opens a signing session for a message the coordinator never announced, consumes the next nonce in its committed sequence, and produces the associated effects and actions.
5. Repeating step 2 lets the emitter drive the local nonce sequence and open sessions at will, and the same construction works for `KeyGenComplained` (which triggers an unconditional share reveal on the validator side) and `TransactionProposed`.

The same construction with an _undecodable_ payload instead of a well-formed one stalls the indexer permanently — that is F-CORE-004, which shares this root cause.

## Considered and rejected

- **"The node's filter already restricts emitters."** It restricts them to the _set_, not to the pairing. `Filter::address(vec)` plus `Filter::event_signature(vec)` is `address ∈ A AND topic0 ∈ T` in `eth_getLogs` semantics; there is no per-address topic constraint in the JSON-RPC filter and the code does not compensate for it (basis 1-3).
- **"`use_client_filtering` re-filters on the client, so it must be stricter."** It applies the same predicate (basis 3), so it is exactly as permissive.
- **"The consumer is expected to check `EventLog::address`."** That is the intended contract, but nothing states it — the `EventLog` doc comment (`events.rs:41-53`) describes `address` only as "The address that emitted the event", with no warning that it is the _only_ authority binding — and the one in-tree consumer that watches untrusted addresses checks it in one handler out of fifteen (basis 9). A safety property that depends on every future handler remembering an unannounced rule is a core-level defect, not just a consumer bug.
- **"The sentinel is affected too."** It is not, today: it watches only `[config.oracle, config.consensus]` (`crates/sentinel/src/main.rs:80`), both protocol contracts, and its two watched ABIs share no selector. The exposure is specific to the validator's oracle list — but it is a property of the deployment, not of the core API, which is why the fix belongs in core.
- **"An operator would only list oracles they control."** The configuration says the opposite: an oracle is an external attestation source whose _results_ the validator honours (basis 8). Trusting it for coordinator messages is a strictly larger trust assumption than the one the operator agreed to, and A1 (trusted operator) covers the operator's own files, not third-party contract code.
- **A related latent hazard, recorded but not claimed here:** `Events::topics` for the multi-enum form concatenates each ABI's selectors without de-duplication (`events.rs:569-575`). If two watched ABIs ever share an event signature — the crate's own test fixture `TokenEvents` does, via `Erc20::Transfer` and `Erc721::Transfer` — `Fetch::MultipleQueries` would issue the duplicate query and `.concat` the same logs twice, producing two entries with an identical `(block, index)` and a `state::Error::BadUpdate` fail-stop at `crates/core/src/state/mod.rs:207-211`. Verified not to occur today: the validator's three ABIs and the sentinel's two declare no shared event signature (`grep -n "event " crates/validator/src/bindings.rs crates/sentinel/src/bindings.rs`). Logged as an observation in `../state/coverage-logs.md#r1`, not as a claim.

## Remediation options

1. **Make the watch list a set of (address, event-set) pairs.** Change `EventWatcher::new(provider, config, addresses)` to take `Vec<(Address, Vec<B256>)>` (or a `BTreeMap<Address, Vec<B256>>`), issue one query per group, and reject at decode time any log whose `(address, topic0)` pair is not in the map. Tradeoff: more `eth_getLogs` requests per block for services with several contracts — mitigated by the existing `MultipleQueries` machinery, which already fans out per topic, and by grouping addresses that share an event set.
2. **Bind the ABI to its contract in the decoder.** Extend `watcher_events!` so each variant carries the address (or an address selector) it is valid for, and have `decode_log` take the emitter and only try the ABIs that address is authorised for. Tradeoff: the macro's call sites must supply the addresses, which are only known at runtime, so this needs a small builder rather than a pure macro; the benefit is that the restriction becomes impossible to forget.
3. **Fail closed at the consumer boundary instead.** Keep the current fetch but have the watcher drop (and count) any log whose emitter is not authorised for its topic0, using a mapping supplied by the service. Cheapest change; it keeps the query shape and only adds a client-side check, which also makes it effective for `use_client_filtering`.
4. **If none of the above is taken**, at minimum document the invariant on `EventLog::address` and on `EventWatcher::new` ("every watched address is trusted to emit every watched event; check `EventLog::address` in every handler"), and add an emitter check to the validator's coordinator and consensus dispatch (R6's finding).

Tests to add:

- `events.rs`: fetch a well-formed watched event emitted by watched address B when only address A is authorised for it, and assert it is not returned (or is returned tagged as unauthorised).
- `events.rs`: assert `Events::topics` is de-duplicated, or that `MultipleQueries` de-duplicates before fanning out, so the `TokenEvents` fixture cannot produce duplicate logs.

## Trail

- Reviewer R1: drafted from lead CORE-H4 (analysis confidence 70%). Every core citation re-opened at commit `2893917`; the consumer-side citations (basis 7-9) were re-opened as well to confirm the affordance is genuinely unused rather than merely undocumented. Scoped deliberately to the core mechanism — the validator-side impact and its severity are lead VAL-H2 and belong to R6. Self-estimate 80% for the core mechanism, 55% that a deployment with an attacker-influenced oracle address exists. No `E1`: read-only run.

## Critic (C-CORE-A)

Method note: I read `events.rs:400-469` and `489-519` and the `watcher_events!` macro (`events.rs:530-594`) before the Claim. My independent reading agrees on every mechanical point and disagrees on the severity attribution.

### Per-claim verdicts — mechanism

All **Supported**:

- `Fetch::SingleQuery` (`events.rs:404-411`) builds one filter with `.address(self.addresses.clone)` and `.event_signature(self.topics.clone)` — an `eth_getLogs` filter is by construction the cross product of the address list and the topic0 list, so any watched address may supply any watched topic0. The same holds for `MultipleQueries` (`:412-421`) and for the client-side predicate on the `ClientFiltered` path (`:458-465`: `self.addresses.contains(&log.address) && log.topic0 .is_some_and(|topic| self.topics.contains(topic))` — two independent tests, `&&`-ed).
- `decode_and_sort` (`:495-506`) passes only `log.topics` and `log.data.data` to `E::decode_log`; `log.inner.address` is copied into the output struct and plays no part in choosing the decode.
- The multi-ABI `watcher_events!` arm (`:577-591`) returns the **first** ABI whose `decode_raw_log` succeeds, in declaration order.
- `crates/validator/src/main.rs:56-57` (`let mut watched = vec![consensus, coordinator]; watched.extend(config.validator.oracles.iter.copied);`) and `crates/validator/src/state/mod.rs:458-462` (only `handle_oracle_result` receives `log.address`) are quoted correctly.

### Whether the core-side finding stands on its own — stated plainly, as asked

**It stands, but only as a Low API-affordance finding; the Medium/High reading depends on the precondition it shares with F-VAL-060, and I have not verified that precondition.**

Two things are true independently of any validator assumption, and they are what remains of this finding on its own:

1. `EventWatcher::new(provider, config, addresses)` offers **no way to express per-address topic scoping**. A consumer that wants "these events from this contract only" cannot say so, and there is no second watcher instance pattern documented either. That is a real missing affordance in `core`, citable at `events.rs:213-222`.
2. The type the crate hands consumers, `EventLog<E>`, carries `address` as an untyped field (`events.rs:44-53`) with nothing — no type, no doc comment, no test — telling a consumer it _must_ be checked. The one production consumer that checks it does so for one handler out of fifteen.

What does **not** stand on core evidence alone is the exploitability. The impact the Claim describes ("an oracle contract can drive the validator's key-generation and signing state machines") requires an address in `config.validator.oracles` belonging to a contract that can be made to emit a `FROSTCoordinator` or `Consensus` selector. I read **F-VAL-060** as instructed: R6 computed `keccak256` over the canonical signatures of all 18 events and all 18 functions in `crates/validator/src/bindings.rs` and found **no `topic0` collisions and no 4-byte selector collisions**, and checked that none of `SentinelOracle.sol`, `SimpleOracle.sol` or `AlwaysApproveOracle.sol` declares a colliding event — so with the contracts in this repository the injection is **not reachable today**. R6's own confidence in the precondition is about 25%, and I have done nothing to raise it: whether a real deployment's `oracles` list contains a third-party or upgradeable contract is a deployment fact, not a code fact, and it is not visible in this checkout.

Per the brief, I will not raise core severity on a validator-side assumption I have not checked.

### Finding verdict

**Plausible** — mechanism verified in full; the trigger that gives it weight is unproven. **Certainty 55%.** **Severity corrected: Medium → Low** _for the core-side finding_. As a `core` defect this is a missing API affordance and a documentation gap, with no demonstrated impact inside `core` itself. **F-VAL-060 is the canonical finding for this defect** and is where the severity call belongs; this file should stay open as the `core`-side half (the missing per-address scoping) and be cross-linked, not merged or deleted. A note to that effect belongs in F-VAL-060 as well.

I also confirm the reviewer's decision to leave `Events::topics` de-duplication as observation O-2 rather than a finding: I re-read `events.rs:568-575` and the sentinel's `watcher_events! { pub enum SentinelEvents { Oracle(SentinelOracleEvents), Consensus(ConsensusEvents) } }` (`crates/sentinel/src/bindings.rs:164-171`) and the validator's three-ABI enum (`crates/validator/src/service/mod.rs:71-80`); R6's collision sweep covers the validator set and finds none. The hazard is real and one macro edit away, but it is not reachable today.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 3 as the cheap fix, option 1 as the right one.**

Option 3 (drop and count any log whose `(address, topic0)` pair is not authorised, using a mapping the service supplies) is the cheapest correct change: it keeps the query shape, needs no trait signature change, and — the finding notes this and it matters — it is the only option that is also effective on the `use_client_filtering` path, where the client already sees every log in the block and filters locally (`index/events.rs:458-465`).

Option 1 (make the watch list `(address, event-set)` pairs) is the structurally right fix and its cost is overstated: `Fetch::MultipleQueries` already fans out per topic, so grouping addresses that share an event set means the request count usually does not change at all.

Option 2 (bind the ABI to its contract inside `watcher_events!`) is sound in intent but the finding correctly identifies why it does not work as a pure macro — the addresses are runtime values. As written it is a builder, not a macro change; the report should say so or it will be costed wrong.

**Dependency worth recording:** F-CORE-004 option 2 (skip undecodable logs) is only safe _with_ this finding fixed, because without per-address topic sets any watched address can emit any watched topic and therefore manufacture the skip. The two should be sequenced: F-CORE-006 first.
