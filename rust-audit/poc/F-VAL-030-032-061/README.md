# PoC — F-VAL-030, F-VAL-032 and F-VAL-061 (the phantom-chunk cluster)

- **F-VAL-030** — a lost or failed `NonceTree` effect leaves a phantom chunk reservation counted as capacity and never retried.
- **F-VAL-032** — a `Sign` whose sequence has no linked nonce chunk permanently discards the signing session.
- **F-VAL-061** — a failed effect is silently converted to `Resume::Noop`, stranding state written in anticipation of it.

They meet at one state value — `NonceState { next_sequence, chunks: { c => None } }` — so they share one harness. F-VAL-061 is the cause, F-VAL-030 the persistence, F-VAL-032 the damage.

> **This code has never been compiled or run.** No Rust toolchain on the audit host.

## 1. Two files, two insertion points

| File | Parent | Covers |
| --- | --- | --- |
| `nonce_state.rs` | `crate::state` | F-VAL-030 (never retried, self-heals only after 1024 sequences), F-VAL-032 (session discarded, both packet kinds, plus the griefing variant), F-VAL-061 state half |
| `effect_failure.rs` | `crate::service` | F-VAL-061 handler half: `NonceTree` with no stream ⇒ `Resume::Noop`, and the ordering that makes it reachable |

Add to `crates/validator/src/state/mod.rs`:

```rust
#[cfg(test)]
#[path = "../../../../rust-audit/poc/F-VAL-030-032-061/nonce_state.rs"]
mod poc_f_val_030_032_061;
```

Add to `crates/validator/src/service/mod.rs`:

```rust
#[cfg(test)]
#[path = "../../../../rust-audit/poc/F-VAL-030-032-061/effect_failure.rs"]
mod poc_f_val_061;
```

`effect::Handler` is not re-exported by `crates/validator/src/service/mod.rs:6-9`, so the second file must be a child of `crate::service` specifically.

## 2. Commands

```sh
cargo test -p validator --bins state::poc_f_val_030_032_061 -- --nocapture
cargo test -p validator --bins service::poc_f_val_061 -- --nocapture
```

`reconciling_first_makes_the_same_effect_succeed` generates a real 1024-nonce chunk and writes 1025 rows in one transaction, so it takes seconds; `NonceGenerator::start_with_sampler`'s small-chunk test sampler is private to `crate::secrets::nonces` and cannot be reached from `crate::service`. Everything else is instant.

## 3. Fixtures

Three-participant genesis group over Anvil #0 (this validator), #1 and #2; `Consensus` at `0x5FbDB…80aa3`; oracle at `0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9`; chain id `31337`; shipped timeout defaults.

The phantom state, spelled out — this is the exact snapshot value the finding describes:

```text
State {
    active_epoch: EpochId::Genesis,
    epochs: { Genesis => Epoch {
        group:     <genesis group>,
        key_share: KeyShare::dummy(),
        nonces:    NonceState { next_sequence: 1024, chunks: { 1 => None } },
    }},
    ..Default::default()
}
```

`available` reads that as `SEQUENCE_CHUNK_SIZE - 0 = 1024`, which is ten times `NONCE_TOPUP_THRESHOLD` (100), so `handle_nonce_topup` returns early forever.

The attacker's input for the griefing case is one literal call, `Coordinator.sign(gid, message)`, which the coordinator turns into `Sign { initiator, gid, message, sid, sequence: 1024 }`. Under A2 that call is permissionless and costs ~150k gas (`crates/validator/src/service/action.rs:300-317`).

## 4. What a pass and a failure mean

| Test | PASS | FAIL |
| --- | --- | --- |
| `a_phantom_reservation_is_counted_as_capacity_and_never_retried` | 2 000 blocks produce no `Effect::NonceTree`. F-VAL-030's "nothing ever repairs it" and F-VAL-061's stranded placeholder are `E1`. | some path re-emits; both findings drop to Low (a transient wasted chunk). Report the block number. |
| `the_phantom_clears_only_after_a_full_chunk_of_sequences` | Recovery needs the _group_ to consume 1024 further sequences — the traffic this validator is failing to serve. This is what makes the impact epoch-scale. | if it clears earlier, F-VAL-030's blast radius is smaller than claimed; recompute it. |
| `an_unlinked_sequence_discards_the_signing_session` | For **both** `Packet::EpochRollover` and `Packet::Transaction`, one `Sign` empties `state.signing` and `state.signature_id_to_message` and emits nothing. F-VAL-032's severity correction (Medium → High) is confirmed: a rollover attestation has no re-proposal path. | the session survives; F-VAL-032 is refuted. |
| `a_permissionless_sign_at_an_unlinked_sequence_grieves_a_healthy_validator` | A _healthy_ validator with a correctly linked chunk 0 loses an honest rollover session to one attacker-chosen sequence. F-VAL-032's griefing trigger needs no pre-existing phantom. | the attacker needs a phantom first; downgrade the griefing trigger to the self-inflicted one. |
| `a_linked_sequence_is_served_normally` | Control. Same state, linked chunk ⇒ `CollectNonceCommitments` and `Effect::RevealNonceCommitments { root, offset: 42 }`. Without it the tests above could pass for the wrong reason. | the harness is wrong, not the code. Fix the harness before trusting anything else here. |
| `resume_noop_is_indistinguishable_from_success` | The serialized state is byte-identical before and after `Resume::Noop`. F-VAL-061's "exactly one failure policy: forget it happened". | — |
| `nonce_tree_without_a_generator_stream_resumes_as_noop` | `Resume::Noop`. F-VAL-030 trigger B and F-VAL-061's handler half are `E1`. | the handler distinguishes failure somehow; F-VAL-061's central claim is wrong. |
| `reconciling_first_makes_the_same_effect_succeed` | The same effect succeeds once the stream exists — proving the defect is the _order_, not the effect. Acceptance test for F-VAL-061 option 4. | — |
| `use_nonce_on_a_missing_nonce_resumes_as_noop` | Included deliberately: this `Noop` is **correct**, and shows F-VAL-061's complaint is scoped to effects whose state is written before they run. | — |

## 5. Known mechanical gaps

- `Epoch`, `NonceState`, `Packet` and `SigningState` are private to `crate::state`; `handle_sign`, `handle_nonce_topup` and `handle_group_reconciliation` are `pub(super)` there. A child module sees all of them; nothing outside `crate::state` does. If any of these change visibility the include point must move with them.
- `NonceState::available` is a plain private `fn` in `state::preprocess`, so it is exercised _indirectly_, through `handle_nonce_topup`'s early return. That is the behaviour that matters, but if you want the direct unit test the findings ask for, `available` needs `pub(super)`.
- `KeyShare::dummy` is `#[cfg(test)] pub(crate)` (`crates/validator/src/frost/keygen.rs:443-453`), so it is available here.

## 6. Remediation check

**F-VAL-030 option 1 (re-emit for a stale `None` reservation) — sound.** Duplicate-safe: a second chunk simply produces a second root and `handle_preprocess` links whichever lands. Respects the `core::state` contract that effects may run more than once. Needs the age of the reservation, which the state does not currently record — say so: it wants a `reserved_at: u64` beside the `None`, or the block-budget scan of F-VAL-061 option 2.

**F-VAL-030 option 2 / F-VAL-061 option 3 (exclude `None` from `available`) — sound and one line, and it is the one to take first.** It makes the threshold mean what it says and converts a permanent stall into "another top-up next block". `a_phantom_reservation_is_counted_as_capacity_and_never_retried` is its acceptance test, inverted. Note the duplicate-suppression argument is correct: `NonceStream::next`'s `Semaphore(1)` already returns `Ok(None)` for a concurrent duplicate (`crates/validator/src/secrets/nonces.rs:195-202`), so the extra top-ups are cheap.

**F-VAL-030 option 3 / F-VAL-061 option 4 (reorder, or start the stream on demand) — the reorder half does not work and the finding says so.** The driver spawns both commands concurrently (`core/driver.rs:266-274`), so emitting `ReconcileGroupSecrets` first only shortens the window. The _on-demand start_ half does work and should be taken: `Effect::NonceTree` should start the group's stream itself when the group has a key share, which removes the cross-effect ordering dependency entirely.

**F-VAL-030 option 4 (carry the chunk index through the effect) — sound, and it is the only option that makes the mismatch visible.** Worth taking together with option 2.

**F-VAL-032 option 1 (re-insert the session in the `(None, Some(WaitingForRequest))` arm) — sound and correct**, and it is the minimal fix: the sequence has already advanced, so the validator simply rejoins at the next `Sign`. Refresh the deadline as the option says, otherwise the session is re-inserted already expired and `handle_signing_timeouts` reaps it on the next block.

**F-VAL-032 option 2 (resolve the nonce before removing) — strictly better than option 1** and it is what I would recommend: `state.signing.get(&event.message)` for the match, `remove` only on the arms that transition. It removes the whole class rather than patching one arm, and it makes the `(_, Some(other))` re-insert at `state/sign.rs:113-119` unnecessary.

**F-VAL-032 option 3 — leave `observe` alone.** The option itself concludes the sequence must advance, and it must: the counter is group-global (`FROSTCoordinator.sol:536`) and a validator that did not advance it would mis-resolve every later sequence. Deferring the _pruning_ side effect is a separate change with its own risk (unbounded `chunks` growth) and should not be bundled in.

**F-VAL-061 option 1 (`Resume::Failed`) — sound and it does not break the runtime contract.** The contract in `crates/core/src/state/mod.rs:56-64` is that transitions are pure and effects may run more than once; carrying the outcome in the resume value keeps both. Two cautions: the transition must stay total — a `Resume::Failed` for a group it no longer tracks has to be a no-op — and any retry it triggers must be bounded, or a persistent SQLite fault becomes an effect-spawn loop.

**F-VAL-061 option 2 (self-healing scan on `NewBlock`) — sound**, and it subsumes F-VAL-004's fix. Its correctness rests on both effects being idempotent at the store level, which is true today (`store_keygen_secrets` is insert-only) — but **only while F-VAL-005/F-VAL-066 are unfixed in the direction that keeps the row**; if a fix ever makes the row deletable mid-ceremony, this retry resamples. Fix F-VAL-005 first, then this.

**F-VAL-061 option 5 (observability) — do it.** The `effects_total` `Success` label explicitly covers "an expected no-op" (`crates/validator/src/metrics.rs:91-92`), so today there is no signal at all that distinguishes the three outcomes.
