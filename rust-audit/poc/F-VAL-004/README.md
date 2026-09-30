# PoC — F-VAL-004

**A single failed or lost `KeyGenSetup` effect during genesis stalls the validator forever: the genesis rollover state has no deadline, no timeout arm and no retry.**

> **This code has never been compiled or run.** No Rust toolchain on the audit host (`state/baseline.md` §1). Identifiers checked against commit `2893917`.

## 1. Wiring it in

`State`, `RolloverState` and `KeyGenCommitment` are private to `crate::state`, and the `validator` crate is a binary with no lib target, so the test must be a `#[cfg(test)]` child of that module. Add to `crates/validator/src/state/mod.rs`:

```rust
#[cfg(test)]
#[path = "../../../../rust-audit/poc/F-VAL-004/genesis_stall.rs"]
mod poc_f_val_004;
```

`crates/validator/src/state/` currently contains **zero tests**, so this file also carries the first harness for that module: a three-participant `ValidatorConfig`, a `Transition`, and a synthetic `Coordinator::KeyGen` log. The other state-machine PoCs in this audit repeat that harness on purpose, so each directory can be applied independently.

## 2. Command

```sh
cargo test -p validator --bins state::poc_f_val_004 -- --nocapture
```

Runtime: seconds. 10 000 pure transitions, no I/O, no chain.

## 3. Fixtures

| Fixture | Value |
| --- | --- |
| this validator | `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` (Anvil #0) |
| peers | Anvil #1, #2 — a 3-participant genesis group, `threshold = 2` |
| `Consensus` address | `0x5FbDB2315678afecb367f032d93F642f64180aa3` (Anvil's first deployment) |
| chain id | `31337` |
| `genesis_salt` | `0x00…00` |
| timeouts | the shipped defaults: `blocks_per_epoch = 1440`, `key_gen_timeout = 120`, `signing_timeout = 6`, `oracle_timeout = 12` (`crates/validator/src/config.rs:87-102`, assumption A10) |
| genesis `KeyGen` log | block 100, emitted by the `Consensus` address, with `gid` and `context` taken from `ParticipantSet::group.parameters` so `handle_genesis_key_gen`'s `event.gid != genesis.id` guard passes |
| blocks driven | 101 … 10 099 — **eight epochs** and eighty-three `key_gen_timeout` windows |

The failure injected is literally the one the code produces: `Message::Resume(Resume::Noop)`. `Handler::perform_effect` returns exactly that for **any** error inside `try_perform_effect` (`crates/validator/src/service/effect.rs:246-252`), so no mocking of `sqlx` is required — the finding's trigger A (an `SQLITE_BUSY` or pool timeout inside `store_keygen_secrets`) and trigger B (a restart losing the resume) are indistinguishable at this boundary, and this test covers both.

## 4. What a pass and a failure mean

| Test | PASS | FAIL |
| --- | --- | --- |
| `a_lost_genesis_setup_stalls_forever` | After 10 000 `NewBlock` transitions the state is still `CollectingCommitments { next_epoch: Genesis, secrets: Participating { secrets: None }, deadline: None, commitments: {} }`, no `Effect::KeyGenSetup` was re-emitted, and no `Action::KeyGenAndCommit` was queued. F-VAL-004 is reproduced deterministically at `E1`; the severity (High — network-wide bootstrap failure, since `FROSTCoordinator` leaves `COMMITTING` only when `--state.pending == 0`) stands. | The assertion names the block number at which a command appeared, or the final `panic!` prints the state the rollover moved to. Either way the finding is **refuted** and its certainty should go to 0 — report which routine recovered. |
| `a_delivered_genesis_setup_publishes_the_commitment` | Control. A delivered `Resume::Setup` does queue `Action::KeyGenAndCommit { expires_at: None, .. }`, and a **second** delivery of the same resume queues nothing. The second half is the safety property remediation option 1 depends on. | If the duplicate resume queues a second action, remediation option 1 ("re-issue the effect every k blocks") would publish duplicate `keyGenAndCommit` transactions and must be revised — say so, because it changes the recommendation. |

## 5. Not covered

- That `store_keygen_secrets` really can return `SQLITE_BUSY` under the shipped pool configuration. That is trigger A's first step and is a property of `sqlx`/SQLite, not of this crate; the test starts one step later, at the `Resume::Noop` the handler produces for _any_ error.
- The onchain half — that one non-committing participant keeps the group in `COMMITTING`. That is Solidity (`contracts/src/FROSTCoordinator.sol:368-372`) and needs `forge`.

## 6. Fix verification

After remediation option 1 (re-issue `KeyGenSetup` while `secrets: None`), `a_lost_genesis_setup_stalls_forever` **must fail** at the `key_gen_setup_effects(&commands) == 0` assertion, and the block number it names is the retry interval — check it against the intended backoff. `a_delivered_genesis_setup_publishes_the_commitment` must keep passing unchanged; it is the guard that the retry does not double-publish.

Remediation option 2 (give genesis a deadline) is **not** safe on its own and this PoC shows why: add a `deadline: Some(..)` to the initial state and the timeout arms become reachable, but for genesis `restart_key_gen_excluding` refuses to restart (`state/keygen.rs:1195-1210`) and the rollover reaches `RolloverState::Halted` — the stall is replaced by a permanent halt. If option 2 is taken, a test asserting that the genesis deadline maps to _re-run setup_ and never to `rollover_failure` is mandatory.
