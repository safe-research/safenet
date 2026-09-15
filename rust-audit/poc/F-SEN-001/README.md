# PoC — F-SEN-001

**Never compiled, never run.** No Rust toolchain on the audit host (`rust-audit/state/baseline.md` §1). Identifiers checked by hand against commit `2893917`; expect mechanical fixes on first build.

## What it shows

A restart (or any reorg within `max_reorg_depth`) makes the sentinel throw away the chain's own evidence that it committed, so at `commit_deadline + 1` it decides "our commit never landed" and **drops the request without revealing**. Onchain the commitment is real and stays `PENDING`, so `slashAmount` is taken the moment any peer finalises, and the remainder is never claimed.

## Where the code goes

`sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` would not compile against it. Paste both tests into the existing `#[cfg(test)] mod tests` block at the bottom of `crates/sentinel/src/service.rs`, immediately before its closing `}`. They reuse that module's helpers verbatim: `transition`, `log`, `proposed_event`, `new_request_event`, `committed_event`, `engine_check_effect`, `resolve_engine_check`, `request_id`, `safe_tx`, `self_address`, `self_signer`, and the `ORACLE` / `TO` / `REASON` / `VOTING_WINDOW` constants.

```
# from the repo root
$EDITOR crates/sentinel/src/service.rs     # paste poc_service.rs before the final }
cargo test -p sentinel --bin sentinel service::tests::poc_f_sen_001
git checkout -- crates/sentinel/src/service.rs
```

The second test additionally needs `sqlx` and `safenet_core::state::StateMachine`, both already dependencies of the `sentinel` crate (`crates/sentinel/Cargo.toml`).

## Fixtures — spelled out

Under A2 the Safe transaction and the chain messages are attacker-controlled, so every input is literal rather than described:

| Fixture | Literal value |
| --- | --- |
| `safeTxHash` | `B256::repeat_byte(0x01)` = `0x0101…01` |
| `epoch` | `7` (fixed by the module's `proposed_event`) |
| `oracle` | `ORACLE` = `0x1111111111111111111111111111111111111111` |
| `consensus` | `CONSENSUS` = `0x3333333333333333333333333333333333333333` |
| `safe` / `to` | `SAFE` = `0x4444…44`, `TO` = `0x5555…55`, `operation = CALL` |
| `chainId` | `1` |
| `requestId` | `oracle_tx_proposal_hash(1, CONSENSUS, 7, ORACLE, b"", 0x0101…01)` |
| `NewRequest` | `fee = 1_000`, `bondTarget = 500`, `slashAmount = 500`, `commitDeadline = 20`, `revealDeadline = 40` |
| `Committed` | `sentinel = self_address`, `bondAmount = 500` |
| Engine verdict | `CheckOutcome::Approved` (so `approve = true`, `reason = ""`) |
| Block sequence | proposal + request at block **10**, own commit at block **12**, then `NewBlock(21)` = `commitDeadline + 1` |

`self_address` is the address of `SigningKey::from_bytes(keccak256("sentinel-flow-test-key"))`, the key the crate's own flow tests already use.

## Reading the result

**Test 1 — `poc_f_sen_001_replayed_own_commit_is_discarded_so_no_reveal_is_emitted`**

- **Passes** → at `NewBlock(21)` the transition emitted exactly one `SentinelActionKind::Reveal { id, approve: true, salt, reason: "" }` with `expires_at: Some(40)`, and the entry advanced to `CollectingVotes`. The finding is fixed.
- **Fails at the `RequestState::WaitingForEngineCheck` assertion** → confirms the intermediate step: `handle_committed` logged `ignoring unexpected commitment` and left no trace. (This assertion should _hold_ on this checkout, so a failure here means the harness is wrong, not the code.)
- **Fails at the final `assert_eq!(commands, vec![Reveal…])` with `commands == []`** → **the finding reproduces.** No `Reveal` for a live onchain commitment. The concrete loss per affected request is `slashAmount` (500 in the fixture, the governed value in production) plus `bondTarget − slashAmount` locked in the oracle until an operator calls `claim` by hand.

**Test 2 — `poc_f_sen_001_warp_page_applies_the_commit_before_the_effect_is_spawned`**

This one is about ordering, not about the loss. It runs the real `StateMachine`.

- **Result on this checkout**: the returned commands are exactly `[Command::Effect(EngineCheck { request_id, transaction, block: 10 })]`. The `Committed` log at block 12 was applied — and discarded — inside `handle_update`'s synchronous per-log loop (`crates/core/src/state/mod.rs:213-223`) **before** `Driver::update` reached the line that spawns effects (`crates/core/src/driver.rs:255` then `:272`).
- **Why that matters**: it removes the finding's only inferential step. The reviewer's basis 6 argued a _race_ between the engine's HTTP round trip and the replayed logs. On the restart path there is no race: warp pages are up to `block_page_size` = 100 blocks (`crates/core/src/index/events.rs:97`) delivered as one `Update::Logs`, so the proposal and the commit are in the same batch for any realistic restart and the commit is always consumed first.

## Remediation check

See the `## QA (QA-CORE-SEN)` section of `rust-audit/findings/F-SEN-001.md`.
