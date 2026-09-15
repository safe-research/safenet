# F-SEN-013 A single undecodable `Revealed.reason` from any active sentinel would stall every other sentinel's indexer permanently — REFUTED by execution: `alloy-sol-types` 1.6.0 decodes invalid UTF-8 lossily, so no stall occurs (Informational)

| Field | Value |
| --- | --- |
| Status | Refuted (as filed); retained as Informational |
| Crate and module | sentinel, bindings.rs (with core: index/events.rs, driver.rs) |
| Location | crates/sentinel/src/bindings.rs:35-44, 164-170 (related: crates/core/src/index/events.rs:491-519, 546-554, crates/core/src/driver.rs:206-225) |
| Severity | Informational / Informational (V-CORE-SEN: basis 8 refuted by execution) |
| Certainty | 98% in the refutation (V-CORE-SEN, executed) |
| Assumptions involved | A2, A6 |
| Tags | dos, input-validation, deps |

## Claim

`SentinelOracle.Revealed` carries a `string reason` and `DisputeResolved` carries a `string context` (`bindings.rs:35-44`). Solidity does not validate that a `string` is UTF-8, so an active sentinel can call `reveal(requestId, approve, salt, reason)` with arbitrary bytes — including an invalid UTF-8 sequence — provided it commits to the same bytes, which is trivial because the commitment is just `keccak256(… ‖ reason)` and is computed by the attacker.

The core watcher decodes a whole log batch with `decode_and_sort`, where **any** log that fails to decode aborts the entire batch with `Error::DecodeLog` (`core/index/events.rs:495-516`), because `watcher_events!`'s `decode_log` maps a decode failure to `None` (`core/index/events.rs:546-554`). The driver treats every watcher error except `ExceededMaxReorgDepth` as transient and retries it every 100 ms **forever** (`core/driver.rs:206-225`). A permanently undecodable log therefore produces a permanent, silent hot loop: the sentinel never advances past that block again, never votes on anything, never reveals a commitment it has already made (so its outstanding bonds are slashed for non-reveal), and never claims. The same log is in every sentinel's and every validator's filter, so the stall is fleet-wide.

**The pivotal claim is unverified.** Whether `alloy-sol-types` 1.6.0 (`Cargo.lock:678-680`) rejects invalid UTF-8 in a `string` field or decodes it lossily (`String::from_utf8_lossy`) cannot be checked in this checkout — the registry is not on disk (A6). If it decodes lossily, this finding is Informational (the reason string is never used by the FSM, so lossy replacement is harmless) and the only residual note is the fragility of the "one bad log stalls the batch" design. If it rejects, this is a High-severity liveness attack available to any single active sentinel for the price of one `reveal`.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The watched event set includes two attacker-supplied `string` fields | E2 | crates/sentinel/src/bindings.rs:35-44 | `            event Revealed(`<br>`                bytes32 indexed requestId,`<br>`                address indexed sentinel,`<br>`                bool approved,`<br>`                uint96 bondAmount,`<br>`                string reason`<br>`            );`<br>`            event DisputeResolved(bytes32 indexed requestId, RequestState outcome, uint128 slashed, string context);`<br>`            event ArbitrationTimedOut(bytes32 indexed requestId);`<br>`            event DisputeOutOfScope(bytes32 indexed requestId, string context);` |
| 2 | Those events are in the watcher's filter, so every sentinel fetches and decodes them | E2 | crates/sentinel/src/bindings.rs:162-170 | `// The event set consumed by the Watcher and StateMachine: all events from`<br>`// both the SentinelOracle and Consensus contracts.`<br>`watcher_events! {`<br>`    #[derive(Debug)]`<br>`    pub enum SentinelEvents {`<br>`        Oracle(oracle::SentinelOracle::SentinelOracleEvents),`<br>`        Consensus(consensus::Consensus::ConsensusEvents),`<br>`    }`<br>`}` |
| 3 | One undecodable log fails the whole batch | E2 | crates/core/src/index/events.rs:495-516 | `    let mut logs = logs`<br>`        .iter`<br>`        .map(\|log\| {`<br>`            E::decode_log(log.topics, &log.data.data)`<br>`                .and_then(\|data\| {`<br>`                    Some(EventLog {`<br>`                        block: log.block_number?,`<br>`                        index: log.log_index?,`<br>`                        address: log.inner.address,`<br>`                        data,`<br>`                    })`<br>`                })`<br>`                .ok_or_else(\|\| Error::DecodeLog {` |
| 4 | A decode failure is discarded into `None`, losing the reason | E2 | crates/core/src/index/events.rs:546-554 | `            fn decode_log(`<br>`                topics: &[::alloy::primitives::B256],`<br>`                data: &[u8],`<br>`            ) -> ::std::option::Option<Self> {`<br>`                <$events as ::alloy::sol_types::SolEventInterface>::decode_raw_log(`<br>`                    topics, data,`<br>`                )`<br>`                .ok`<br>`            }` |
| 5 | Every watcher error but the deep-reorg one is retried every 100 ms indefinitely | E2 | crates/core/src/driver.rs:206-225 | `                match self.watcher.next.await {`<br>`                    Ok(update) => return Ok(update),`<br>`                    Err(`<br>`                        err @ index::Error::Blocks(index::blocks::Error::ExceededMaxReorgDepth(_)),`<br>`                    ) => {`<br>`                        return Err(err);`<br>`                    }`<br>`                    Err(err) => {`<br>`                        tracing::warn!(`<br>`                            ?err,`<br>`                            "failed to get next blockchain update; retrying after delay"`<br>`                        );`<br>`                        tokio::time::sleep(STEP_RETRY_DELAY).await;`<br>`                    }`<br>`                }` |
| 6 | The reason is written verbatim by the revealing sentinel and only checked against the commitment hash, never validated | E2 (Solidity reference, A7) | contracts/src/SentinelOracle.sol:239-243 | `    function reveal(bytes32 requestId, bool approve, bytes32 salt, string calldata reason) external {`<br>`        SentinelOracleRequest.T storage request = $requests.get(requestId);`<br>`        $commitments.reveal(requestId, msg.sender, approve, salt, reason);`<br>`        request.applyReveal(approve);`<br>`    }` |
| 7 | The commitment is a plain packed keccak over the same bytes, so committing to non-UTF-8 is trivial | E2 | contracts/src/libraries/SentinelOracleCommitments.sol:47-56 | `    function computeHash(address sentinel, bytes32 requestId, bool approve, bytes32 salt, string calldata reason)`<br>`        internal`<br>`        pure`<br>`        returns (bytes32)`<br>`    {`<br>`        // \`reason\` is appended last since it's the only variable-length field in the packed`<br>` // encoding, keeping the preimage unambiguous.`<br>` // forge-lint: disable-next-line(asm-keccak256)`<br>` return keccak256(abi.encodePacked(approve, salt, sentinel, requestId, reason));`<br>` }` |
| 8 | Whether `alloy-sol-types` 1.6.0 rejects invalid UTF-8 in a `string` field | **I** — not verifiable in this checkout | Cargo.lock:678-680 | `name = "alloy-sol-types"`<br>`version = "1.6.0"`<br>`source = "registry+https://github.com/rust-lang/crates.io-index"` |

## Trigger

Requires one address that governance has added as an active sentinel — i.e. an actor inside the A2 fault bound:

1. The attacker picks any live request and computes `hash = keccak256(abi.encodePacked(approve, salt, attacker, requestId, reason))` with `reason` a byte string containing an invalid UTF-8 sequence (for example a lone `0x80` continuation byte). No offchain tooling restriction applies — `abi.encodePacked` on a `string` is just its bytes (basis 7).
2. The attacker calls `commit(requestId, hash)` inside the commit window, then `reveal(requestId, approve, salt, reason)` inside the reveal window (basis 6). Total cost: one bond (returned or partially slashed like any other vote) plus gas.
3. The `Revealed` log lands in the block. Every sentinel and validator watching `SentinelOracle` fetches it in its next `eth_getLogs` batch.
4. **If basis 8 holds**, `decode_raw_log` returns `Err` → `decode_log` returns `None` (basis 4) → the whole batch fails with `Error::DecodeLog` (basis 3) → the driver retries the same range every 100 ms forever (basis 5). Every affected process stops advancing at that block permanently, with only a repeating `warn`. Restarting does not help: the log is still there.
5. Secondary damage: every sentinel with an outstanding commitment stops revealing, so all of those bonds are slashed for non-reveal (`contracts/src/libraries/SentinelOracleRequests.sol:289-296`).

If basis 8 does not hold (lossy decoding), steps 4 and 5 do not occur and the reason simply arrives with replacement characters; the FSM ignores it entirely (`handle_revealed` never reads `event.reason`, `service.rs:335-385`), so there is no impact.

## Considered and rejected

- **"The `reason` is bounded to `R-<u32>.<u32>`, so it cannot be arbitrary."** That bound applies only to the reason _this_ sentinel produces from its own engine (`engine.rs:37-49`, `service.rs:175`). `Revealed.reason` is whatever another sentinel wrote, and is fully attacker-controlled under A2.
- **"Only the emitting contract's own events reach the decoder."** True and irrelevant: `Revealed` is a `SentinelOracle` event and the oracle is one of the two watched addresses (`main.rs:80`).
- **"Client-side filtering (`use_client_filtering`) would avoid it."** It changes how logs are fetched, not how they are decoded; `decode_and_sort` runs either way.
- **"The driver would eventually exit."** It would not. Only `ExceededMaxReorgDepth` breaks the retry loop (basis 5); `DecodeLog` is classified as transient. `M9` in the codebase map records the same pattern from the core side.
- **"A restart clears it."** The undecodable log is a permanent feature of the canonical chain; every restart re-fetches it.
- **"Solidity would reject non-UTF-8."** Solidity's `string` is `bytes` with a type tag; there is no validation at any layer, and `abi.encodePacked` (basis 7) treats it as raw bytes.
- **Why this is filed rather than dropped:** every step except basis 8 is E2, the trigger is concrete and cheap, and the consequence if basis 8 holds is a total, unrecoverable liveness failure of the whole sentinel and validator fleet. The codebase map records this as the crate's one open check; leaving it as an unverified observation would risk it being silently dropped.

## Remediation options

1. **Settle basis 8 first (QA).** With the registry available, run a two-line test: build a `SentinelOracle::Revealed` log whose `reason` field encodes `[0x80]` and assert on `SentinelOracleEvents::decode_raw_log(...)`. Everything below is conditional on that result.
2. **Do not let one log poison a batch.** In `decode_and_sort`, skip (with a `warn` and a counter) logs that fail to decode instead of returning `Error::DecodeLog`, or return the successfully decoded ones alongside a diagnostic. Safe for the sentinel, which uses no `string` field for anything, but it is a core change that must be weighed against services that need every log (a validator missing an attestation is not the same as a sentinel missing a reason).
3. **Reclassify `DecodeLog` as fatal rather than transient.** Retrying a deterministic decode failure forever converts a bug into a silent stall; exiting loudly at least surfaces it. Tradeoff: turns a stall into an outage, which is only an improvement if paired with alerting.
4. **Drop the `string` fields from the decoded shape.** Declare `reason` and `context` as `bytes` in `bindings.rs` (the ABI encoding of `string` and `bytes` is identical, so the topic0 selector is unaffected only if the _signature_ is unchanged — it is not, so this requires decoding the data manually rather than via `sol!`). Mentioned for completeness; option 2 is cleaner.
5. **Alert on a frozen indexer.** `safenet_core_block_number{status="processed"}` (`core/driver.rs:313-317`) stops advancing during the stall; an alert on that gap turns an invisible failure into a paged one regardless of the outcome of option 1.

Tests to add: the decode test in option 1; a watcher test asserting the batch behaviour for one undecodable log among several; an integration-script scenario where one sentinel reveals a non-UTF-8 reason and the other keeps making progress.

## Trail

- Reviewer R7: drafted from the codebase map's "one open check" for this crate, self-estimate 45%. Basis 1 to 7 were re-opened in this checkout and are E2. Basis 8 — the alloy behaviour on which everything turns — is class `I` and explicitly not verifiable here (no registry on disk, A6). The certainty should sit in the 40 to 69 band until QA settles basis 8, then move to the top or to 0.

## Critic (C-SEN)

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `bindings.rs:35-44` verbatim; `Revealed.reason` and `DisputeResolved.context`/`DisputeOutOfScope.context` are `string`. |
| 2 | **Supported** | `bindings.rs:162-170` verbatim; the `watcher_events!` set covers both contracts, and `main.rs:80` puts the oracle in the address filter. |
| 3 | **Supported** | `core/index/events.rs:495-516` verbatim. I read the whole function: the `.map(...).collect::<Result<Vec<_>, _>>?` means a single `None` from `decode_log` aborts the entire batch with `Error::DecodeLog`. |
| 4 | **Supported** | `core/index/events.rs:546-554` verbatim; `decode_raw_log(...).ok` discards the error. |
| 5 | **Supported** | `core/driver.rs:206-225` verbatim; only `ExceededMaxReorgDepth` escapes the loop, everything else sleeps `STEP_RETRY_DELAY` (100 ms, `driver.rs:26`) and retries the same step forever. |
| 6 | **Supported** | `SentinelOracle.sol:239-243` verbatim; no validation of `reason` beyond the commitment hash check. |
| 7 | **Supported** | `SentinelOracleCommitments.sol:47-56` verbatim; `abi.encodePacked` over the raw string bytes, so committing to arbitrary bytes is trivial. |
| 8 | **Correctly classified `I` — upholding that classification** | See below. |

### Basis 8 stays class `I`, and I verified the reason rather than taking it on trust

I checked for the source myself: `~/.cargo/registry/src` does not exist, there is no `vendor/` directory, and a filesystem-wide search for any `alloy-sol-types` source file returns nothing. `Cargo.lock:678-681` pins `alloy-sol-types` 1.6.0 from the registry with only a checksum. Under PROMPT.md §2 and the critic brief's §2, an assertion about a pinned dependency's internals with no source on disk is class `I` at best, and A6 puts library internals out of review scope. R7's call is right and I am explicitly **refusing to upgrade it to `E2`** — no amount of protocol reasoning about what a decoder "should" do substitutes for reading it.

### The mechanism is confirmed; only the trigger is unproven

This distinction matters for the number. Steps 1-3 and 5 of the trigger are `E2`: an active sentinel can commit and reveal arbitrary bytes for the price of one bond, the log lands in every watcher's filter, and _if_ the decode fails the stall is permanent, silent and fleet-wide (with the secondary consequence that every outstanding commitment goes unrevealed and is slashed under `SentinelOracleRequests.sol:202-205`, `:289-296`). The single unknown is whether alloy's `string` detokenizer is checked or lossy. That is precisely the rubric's "`I` with a Confirmed mechanism" row, and 45% sits at the bottom of that band when the mechanism is fully verified and the blast radius is the whole fleet. I am raising it to **65%**, which is where a confirmed mechanism with one unproven trigger belongs.

### Overlap with F-CORE-004 — cross-referenced, not merged

The core defect (a deterministic, content-dependent decode failure retried forever with no terminal state) is **F-CORE-004**, which should be **canonical** for the mechanism. This file is canonical for the _attack path_: F-CORE-004 argues from a malformed log arriving somehow; F-SEN-013 supplies the concrete, cheap, in-fault-bound actor (A2) who can put one there deliberately via `SentinelOracle.reveal`, and the sentinel-side secondary loss. Both should survive; each should name the other.

### Finding verdict

**Plausible. Certainty 65%. Severity High if basis 8 holds, otherwise Informational (unchanged).**

I am keeping R7's conditional severity rather than collapsing it, because the two branches differ by four levels and a single unresolved fact decides between them. If confirmed, it is High under Section 8 ("an honest validator or sentinel loses liveness under attacker-controlled input"), and a case could be made for Critical since it stalls the whole fleet and slashes every open bond as a side effect — QA should re-argue that once basis 8 is settled. If refuted, it drops to Informational: `handle_revealed` never reads `event.reason` (I checked `service.rs:335-385` line by line), so lossy replacement characters have no effect on the FSM at all.

### Exactly what QA must read to settle basis 8

With the registry available, this is a source read, not an experiment — but do both:

1. **Read** `alloy-sol-types-1.6.0/src/types/data_type.rs` — the `impl SolType for sol_data::String` block, specifically its `detokenize` (and `valid_token`/`type_check` if present). The question is one line: does it go through `String::from_utf8` (checked → `Err`), or `String::from_utf8_lossy` / `from_utf8_unchecked` (lossy → `Ok`)? Historically alloy has used the lossy path, which would refute the finding — do not assume it, read it.
2. **Also read** `alloy-sol-types-1.6.0/src/types/event.rs` for `SolEvent::decode_raw_log` / `SolEventInterface::decode_raw_log`, to confirm a `detokenize` error propagates as `Err` rather than being swallowed.
3. **Then execute** the two-line test in remediation option 1: build a `Revealed` log whose data segment encodes a `string` of `[0x80]` and assert on `SentinelOracle::SentinelOracleEvents::decode_raw_log(&topics, &data)`. That single assertion moves this finding to 89% or to 0.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain), and cannot be attempted here on the point that matters.** Basis 8 needs the `alloy-sol-types` 1.6.0 source or a running decoder, and neither exists on this host: `~/.cargo/registry` does not exist, there is no `vendor/` directory, and there is no network (`state/baseline.md` §1-2, assumption A6). **I uphold C-SEN's refusal to upgrade basis 8 to `E2`,** and I decline to upgrade it myself on any recollection of what alloy's string detokenizer does.

**Certainty: unchanged at 65%.** Severity stays the Critic's conditional. This finding is not mine to move; it is a single fact away from 89% or 0, and the fact is one command.

### The exact check that settles it

It is question **2** in `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` — QA-XC had already written a ready-to-run version against the crate's real `SentinelOracleEvents` bindings with the event data segment hand-encoded, so I did not duplicate it. I appended two points that version was missing:

1. **Do the source read as well as the run**, because C-SEN asked for it by name and because the read keeps explaining the answer after a dependency bump that the assertion alone would only flag afterwards. Two items: `~/.cargo/registry/src/*/alloy-sol-types-1.6.0/src/types/data_type.rs`, the `impl SolType for sol_data::String` block — does `detokenize` go through `String::from_utf8` (checked → `Err`) or `String::from_utf8_lossy` / `from_utf8_unchecked` (lossy → `Ok`)? Then `…/src/types/event.rs`, `SolEvent::decode_raw_log` / `SolEventInterface::decode_raw_log`, to confirm a `detokenize` error propagates rather than being swallowed.
2. **An `Ok` answer does not close F-CORE-004.** See below.

The attacker's input, spelled out, since A2 makes it in scope: any active sentinel calls `reveal(requestId, approve, salt, reason)` with `reason = [0x80]` — a lone UTF-8 continuation byte, never valid on its own — having committed to `keccak256(abi.encodePacked(approve, salt, sentinel, requestId, [0x80]))`, which it computes itself (`contracts/src/libraries/SentinelOracleCommitments.sol:47-56`). Cost: one bond and one transaction.

### What I could verify here, and did

Basis rows 1-7 are `E2` and I re-opened each. Two additions:

- I confirmed the FSM never reads `event.reason`: `handle_revealed` (`service.rs:335-385`) binds `requestId`, `approved` and `sentinel` and nothing else, and `grep -n "\.reason" crates/sentinel/src/` outside `commit_vote`/`handle_block_advance` (which use the _locally stored_ reason, not the event's) returns nothing. So the lossy branch really is harmless to the sentinel's behaviour, as C-SEN says.
- I confirmed the blast radius is fleet-wide rather than sentinel-only: `SentinelEvents` puts the oracle in the address filter (`main.rs:80`), and the validator watches `Consensus` at the same addresses — so a log that fails to decode inside `decode_and_sort` aborts the batch for **any** service whose filter matches it (`index/events.rs:495-516`). The stall is not attacker-scoped.

### Remediation check

**Option 2 (skip undecodable logs in `decode_and_sort`) must NOT be taken as a blanket `core` change.** The option's own text hedges ("safe for the sentinel … but it is a core change that must be weighed"); I want to state the objection sharply, because it is the same change F-CORE-004 option 2 proposes and the two together will look like consensus. `decode_and_sort` is shared by every service. Silently dropping a log a **validator** needed — a `Sign`, a `KeyGenSecretShared`, a `Preprocess` — is precisely the F-CORE-002 failure mode of committing an incomplete batch as complete, and it is the failure mode this audit rates High. If skipping is adopted at all, the boundary must be explicit: only for a log that _cannot_ be a valid protocol message, only paired with F-CORE-006's per-address topic sets so an unauthorised emitter cannot manufacture the condition, and never in a way that lets an empty or short result pass a completeness gate.

**Option 3 (reclassify `DecodeLog` as fatal rather than transient) is the sound direction**, and it is F-CORE-004 option 1 restated. Retrying a deterministic failure every 100 ms forever converts a bug into a silent stall; a loud exit is strictly better. It only helps if the exit is visible, which today it is not — `Driver::run` discards its outcome and the process exits with status **0** (F-CORE-030). **Option 3 depends on F-CORE-030 landing first.**

**Option 4 (declare the fields as `bytes`) does not work and the finding says so** — changing the Solidity type changes the event signature and therefore `topic0`, so the watcher would stop matching the event entirely. Correctly listed "for completeness"; the report should not carry it as a live option.

**Option 5 (alert on a frozen indexer) is necessary regardless of the answer to basis 8** and is the cheapest thing on the list: `safenet_core_block_number{status="processed"}` already exists (`driver.rs:313-317`) and stops advancing during the stall. It is also the only remediation here that helps for a stall nobody anticipated.

**Recommendation:** settle basis 8 first — nothing else should be decided before it. Then option 5 unconditionally, and option 3 (with F-CORE-030) if the answer is `Err`. Option 2 only with the boundary above written down, and it belongs in F-CORE-004, which is canonical for the mechanism.

**F-CORE-004 is not settled by this answer.** Its mechanism — a deterministic, content-dependent decode failure retried forever with no terminal state — survives a lossy decoder untouched; only the cheap, attacker-chosen path into it goes away. A green run on question 2 must not be read as "the batch-poisoning design is fine".

## Verification (V-CORE-SEN, Phase 5)

**Executed. Basis 8 is REFUTED. The High-severity attack does not exist. Severity → Informational.**

This settles **question 2** of `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`, the audit's highest-value open question and the one that decided this finding's conditional severity. Both halves that QA-CORE-SEN's addendum asked for were done: the dependency source was **read**, and the behaviour was **executed**.

### 1. Source read — `alloy-sol-types` 1.6.0

`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/alloy-sol-types-1.6.0/src/types/data_type.rs:368-388`, `impl SolType for String`:

```rust
    #[inline]
    fn valid_token(token: &Self::Token<'_>) -> bool {
        core::str::from_utf8(token.as_slice).is_ok
    }

    #[inline]
    fn detokenize(token: Self::Token<'_>) -> Self::RustType {
        // NOTE: We're decoding strings using lossy UTF-8 decoding to
        // prevent invalid strings written into contracts by either users or
        // Solidity bugs from causing graph-node to fail decoding event
        // data.
        RustString::from_utf8_lossy(token.as_slice).into_owned
    }
```

The checked path exists but lives in `valid_token`, and `valid_token` is reached **only** through the `*_validate` family. `src/types/event/mod.rs:185-195`:

```rust
    fn decode_raw_log<I, D>(topics: I, data: &[u8]) -> Result<Self> { … Self::abi_decode_data(data)? … }
```

`abi_decode_data` calls `abi_decode_sequence` (**no** validation); only `decode_raw_log_validate` calls `abi_decode_data_validate` → `abi_decode_sequence_validate`. `watcher_events!` generates `SolEventInterface::decode_raw_log` — the **non-validating** one (`crates/core/src/index/events.rs:546-554`) — so the lossy `detokenize` is the path this codebase takes. That is _why_ the answer is `Ok`, and it survives a dependency bump only as long as alloy keeps that comment.

### 2. Executed

A test was appended to `crates/sentinel/src/bindings.rs` (saved as `rust-audit/poc/Q2-utf8/poc_bindings_q2.rs`, output in `rust-audit/poc/Q2-utf8/RESULT-V-CORE-SEN.out`) and run with

```
cargo test -p sentinel --bin sentinel poc_q2_utf8 -- --nocapture
```

using QA-XC's exact log bytes — a `reason` of length 1 whose single byte is `0x80`, a lone UTF-8 continuation byte that is never valid on its own. Verbatim:

```
running 2 tests
DisputeResolved.context  => Ok(DisputeResolved(DisputeResolved { requestId: 0x…aa,
    outcome: RESOLVED_APPROVED, slashed: 0, context: "\u{fffd}" }))
DisputeOutOfScope.context => Ok(DisputeOutOfScope(DisputeOutOfScope { requestId: 0x…aa,
    context: "\u{fffd}" }))
Revealed / SolEventInterface::decode_raw_log => Ok(Revealed(Revealed { requestId: 0x…aa,
    sentinel: 0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266, approved: true, bondAmount: 0,
    reason: "\u{fffd}" }))
Revealed / watcher_events!::decode_log       => Some(Oracle(Revealed(Revealed { …,
    reason: "\u{fffd}" })))
test bindings::poc_q2_utf8::q2_non_utf8_dispute_contexts_decode ... ok
test bindings::poc_q2_utf8::q2_non_utf8_reason_decode ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out
```

The second line of each pair is the one that matters: the check was run **through the exact path `safenet-core` uses** — `watcher_events!`'s `Events::decode_log`, the function whose `.ok` would turn a decode error into the `None` that poisons a batch — and it returned `Some`, not `None`. All three attacker- or arbitrator-supplied `string` fields in the watched set (`Revealed.reason`, `DisputeResolved.context`, `DisputeOutOfScope.context`) decode to U+FFFD and continue.

### Consequence

- **Basis 8 is refuted, not merely unresolved.** `alloy-sol-types` 1.6.0 decodes invalid UTF-8 **lossily** in a `string` field. Its class moves from `I` to `E1` with the opposite value.
- The described attack — one `reveal` with non-UTF-8 bytes stalling every sentinel's and every validator's indexer forever — **does not exist on this checkout**. Nothing fails to decode, so no batch is poisoned and no retry loop is entered.
- Severity is therefore resolved to the branch C-SEN named: the FSM never reads `event.reason` (`crates/sentinel/src/service.rs:335-385`), so a replacement character is harmless. **Severity: High-if-confirmed / conditional → Informational.**
- **This does not close F-CORE-004**, per QA-CORE-SEN's addendum, and I uphold that. F-CORE-004 is canonical for the _mechanism_ — a deterministic, content-dependent decode failure retried every 100 ms forever with no terminal state (`index/events.rs:495-516`, `driver.rs:206-225`) — and the mechanism is untouched by a lossy decoder. Only the cheap, attacker-chosen path into it is gone. A malformed length prefix, a short data field or a future event type still reaches it.
- **Remediation option 2 (skip undecodable logs) should not be adopted here.** It is now unnecessary for this finding, and QA-CORE-SEN's warning stands on its own merits: `decode_and_sort` is shared by every service, and silently dropping a log a validator needed is precisely the F-CORE-002 failure mode — committing an incomplete batch as complete — which this audit's now-`E1` F-CORE-002 rates High.

**Basis class:** `E1` (source read plus execution). **Certainty: 65% → 98%** — that is 98 % confidence in the _refutation_: invalid UTF-8 does not stall the indexer. **Status: Critiqued → Refuted (as filed); retained as Informational** for the residual note about batch fragility, which F-CORE-004 carries.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID as a refutation** — the refuted mechanism is unchanged, and the new events do not resurrect it

**Merged-code citations:**

| Cited at `2893917` | Now at | Changed? |
| --- | --- | --- |
| `bindings.rs:35-44` (`Revealed` with `string reason`, `DisputeResolved` with `string context`) | `bindings.rs:35-41` (`Revealed`) and `:43` (`DisputeResolved`) | the events themselves are unchanged; three new event declarations were inserted between and after them at `:42`, `:44`, `:45` |
| `bindings.rs:164-170` (`watcher_events!`) | `bindings.rs:167-173` | +3, otherwise identical |
| `crates/core/src/index/events.rs:491-519`, `:546-554`; `crates/core/src/driver.rs:206-225` | unchanged | `crates/core` is byte-identical across the merge |

`alloy-sol-types` is unchanged in `Cargo.lock` for this merge, so the executed refutation — lossy UTF-8 decoding, no stall — still holds.

The merge adds two further attacker- or arbitrator-influenced dynamic fields to the sentinel's decoded event set: `event OracleResult(bytes32 indexed requestId, address indexed sponsor, bytes result, bool approved)` (`bindings.rs:45`) and the already-present `string context` on `DisputeResolved`. Both decode through the same `alloy-sol-types` path the refutation exercised — `bytes` has no validity constraint at all, and `string` decodes lossily — so neither reopens the stall. `handle_oracle_result` (`service.rs:775-817`) reads only `event.requestId` and never inspects `event.result`, so no downstream parsing of the new `bytes` field exists to fail either.

**Certainty 98% in the refutation, unchanged. Severity Informational, unchanged.** Status left at `Refuted (as filed); retained as Informational`.
