# PoC — F-SEN-002

**Never compiled, never run.** No Rust toolchain on the audit host (`rust-audit/state/baseline.md` §1). Identifiers checked by hand against commit `2893917`; expect mechanical fixes on first build.

## What it shows

A peer sentinel whose engine answers faster than ours makes our `committed_count` undercount the oracle's real `committedCount`. That undercount fires the early-finalise trigger one reveal too soon, and `finalize` then deletes a **bonded** request with no `Finalize` and no `Claim`. Nothing re-creates it, so `claim` is never called and `bondTarget` stays parked in the oracle.

No restart, no reorg, no attacker. Under A4 there is a second, latency-free trigger: one `Committed` log missing from an incomplete `eth_getLogs` response produces the identical undercount.

## Where the code goes

`sentinel` is a **binary-only crate** (no `lib.rs`), so there is no library target for an integration test. Paste both tests into the existing `#[cfg(test)] mod tests` block at the bottom of `crates/sentinel/src/service.rs`, immediately before its closing `}`. They reuse that module's `transition`, `log`, `proposed_event`, `new_request_event`, `committed_event`, `revealed_event`, `engine_check_effect`, `resolve_engine_check`, `request_id`, `self_address`, `self_signer` helpers and the `ORACLE` / `OTHER` / `TO` / `REASON` constants.

```
# from the repo root
$EDITOR crates/sentinel/src/service.rs     # paste poc_service.rs before the final }
cargo test -p sentinel --bin sentinel service::tests::poc_f_sen_002
git checkout -- crates/sentinel/src/service.rs
```

## Fixtures — spelled out

| Fixture | Literal value |
| --- | --- |
| `safeTxHash` | `0x0202…02` (test A), `0x0303…03` (test B) |
| `epoch` / `oracle` / `consensus` / `chainId` | `7` / `0x1111…11` / `0x3333…33` / `1` |
| `requestId` | `oracle_tx_proposal_hash(1, CONSENSUS, 7, ORACLE, b"", safeTxHash)` |
| `NewRequest` | `fee = 1_000`, `bondTarget = 500`, `slashAmount = 500`, `commitDeadline = 20`, `revealDeadline = 40` |
| Peer B | `OTHER` = `0x8888888888888888888888888888888888888888` |
| Engine verdict (ours) | `CheckOutcome::Approved` → `approve = true`, `reason = ""` |
| Peer B's vote | `approved = true` — deliberately **unanimous**, so the request is undisputed and the loss cannot be blamed on arbitration |
| Block order (test A) | 1 proposal + request · **2 `Committed(B)`** · 3 our engine resume · 4 `Committed(self)` · 21 `NewBlock` (= `commitDeadline+1`) · 22 `Revealed(B)` · 23 `Revealed(self)` |
| Block order (test B) | identical through block 21, then 22 `Revealed(self)` — we reveal first |

## Reading the result

**Test A — `poc_f_sen_002_peer_commit_before_engine_verdict_loses_the_claim`**

- **Fails at the `committed_count: 1` assertion** → the harness is wrong, not the code; this assertion is expected to _hold_ on this checkout and is there to make the undercount visible (local tally 1, onchain `committedCount` 2).
- **Fails at `assert!(state.0.contains_key(&id))` after block 22** → **the finding reproduces.** `handle_revealed` ran the early-finalise trigger on peer B's reveal, `finalize` took the `!self_revealed && !timed_out` branch (`service.rs:626-633`), and the entry was deleted with **no actions at all**.
- **Fails at the final `Finalize` + `Claim` assertion with `commands == []`** → the same defect one step later: our own `Revealed` at block 23 found no entry and was ignored (`service.rs:340-346`). This is the point at which the bond is definitively unrecoverable without manual intervention.
- **Passes** → the sentinel counted peer commitments in every pre-commit phase (or stopped early-finalising on a local tally), and emitted `Finalize` + `Claim` for its bonded, revealed, winning vote. Fixed.

Concrete loss on a pass-to-fail transition: `bondTarget` = 500 in the fixture (the governed amount in production) **plus** the fee share for the winning side, left in the oracle. Not slashed — recoverable by a manual `claim`, if anyone notices. Nothing logs it: the drop at `service.rs:631-633` is silent and no metric distinguishes it (`requests_resolved_total` is only incremented on the branches that _do_ act, `service.rs:663`).

**Test B — `poc_f_sen_002_undercount_finalizes_before_the_request_is_finalisable`**

- **Fails at `assert!(commands.is_empty)`** → the finding reproduces in its mirror form: `Finalize` + `Claim` were emitted at block 22 while peer B's commitment was still unrevealed onchain. Both are guaranteed to revert — `finalize` with `FinalizeTooEarly` (`contracts/src/libraries/SentinelOracleRequests.sol:175-177`) and `claim` with `RequestNotResolved` (`:239-245`) — and the entry is deleted regardless, so a later `DisputeResolved` is ignored too (`service.rs:502-508`).
- **Passes** → the early-finalise trigger no longer fires on an undercounted tally.

## Remediation check

See the `## QA (QA-CORE-SEN)` section of `rust-audit/findings/F-SEN-002.md`.
