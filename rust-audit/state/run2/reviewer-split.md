# Run 2 — reviewer split (Manager's assignment)

Derived from `state/run2/logs/06-inventory.txt` at commit `3ec8bc5`: **61 in-scope `.rs` files, 20,301 lines** across `crates/core` (7,701), `crates/validator` (8,728) and `crates/sentinel` (3,872). Every file is assigned to exactly one reviewer; the Coverage Critic verifies against this table. `crates/sentinel-engine` (22 files, 5,113 lines) is out of scope. Reviewer roles, evidence discipline and boundaries: `state/run2/reviewer-brief.md`.

| Reviewer | Scope | Files | Lines | Finding IDs | Start from |
| --- | --- | --: | --: | --- | --- |
| R1 | core indexing and reorgs | 6 | 4,106 | `F2-CORE-001..029` | CORE-H1, H2, H4, H8, H9, H14; core checklist 1, 3, 4, 8, 9 |
| R2 | core runtime, state, effects, observability | 12 | 2,055 | `F2-CORE-030..059` | CORE-H3, H5, H10, H12, H13, M9, M10; core checklist 2, 10, 11, 12, 14. Note: `driver.rs` and `effects.rs` gained housekeeping wiring in the pruning merge |
| R3 | core transaction queue | 5 | 1,540 | `F2-CORE-060..089` | CORE-H6, H7, H11; core checklist 5, 6, 7 |
| R4 | validator DKG path | 10 | 3,228 | `F2-VAL-001..029` | VAL-H1, H4, H5, H7, H9, M1, M3; validator checklist 1, 3, 7, 8, 12. Apply A16 when rating genesis-only liveness |
| R5 | validator signing path and secrets | 10 | 3,329 | `F2-VAL-030..059` | VAL-H3, H6, H8, H11, M4, M5, M6, M7; validator checklist 3, 4, 5, 10. **`secrets/store.rs` is new territory**: the scheduled-secret-pruning series (+668 lines) landed after the map was written — reorg/crash consistency of scheduling and collection is the priority. Apply A17 |
| R6 | validator service, wiring, config | 8 | 2,171 | `F2-VAL-060..089` | VAL-H2, H7, H10, CORE-H4; validator checklist 2, 6, 9, 11, 13. Also `crates/validator/validator.sample.toml`, `crates/validator/Dockerfile`. `service/effect.rs` gained the housekeeping effect in the pruning merge |
| R7 | sentinel (whole crate) | 10 | 3,872 | `F2-SEN-001..049` | SEN-H1 to H15, M8; sentinel checklist 1 to 13. Also `crates/sentinel/sentinel.sample.toml`, `crates/sentinel/Dockerfile`. The engine is an opaque HTTP oracle: audit the sentinel's handling of its responses, timeouts and replay, not the engine. `service.rs` (+371) and `state.rs` (+207) changed after the map: verdict aggregation, meta-transactions, waiting-for-outcome states |
| R8 | cross-cutting | 0 | 0 | `F2-XC-001..049` | VAL-H10, SEN-H14, SEN-H15, CORE-H17; `Cargo.toml`, `Cargo.lock`, the three crates' `Cargo.toml`, Dockerfiles and sample configs; repo-wide sweeps over the 61 in-scope files for secrets reaching `Debug`/logs/metrics and for panics/casts; `cargo audit` **reachability** and `cargo tree -d` from `state/run2/logs/`; CI gaps |

| **Total** | | **61** | **20,301** | | |

## Files per reviewer

### R1 — core indexing and reorgs (4,106 lines)

- `crates/core/src/index/blocks.rs` (1330 lines, 23 tests)
- `crates/core/src/index/events.rs` (1516 lines, 19 tests)
- `crates/core/src/index/bloom.rs` (523 lines, 2 tests)
- `crates/core/src/index/clock.rs` (103 lines, 2 tests)
- `crates/core/src/index/mod.rs` (468 lines, 5 tests)
- `crates/core/src/provider/mod.rs` (166 lines, 0 tests)

### R2 — core runtime, state, effects, observability (2,055 lines)

- `crates/core/src/driver.rs` (328 lines, 0 tests)
- `crates/core/src/effects.rs` (267 lines, 8 tests)
- `crates/core/src/kdf.rs` (81 lines, 4 tests)
- `crates/core/src/lib.rs` (25 lines, 0 tests)
- `crates/core/src/metrics.rs` (90 lines, 0 tests)
- `crates/core/src/observability/logging.rs` (21 lines, 0 tests)
- `crates/core/src/observability/metrics.rs` (80 lines, 1 tests)
- `crates/core/src/observability/mod.rs` (94 lines, 2 tests)
- `crates/core/src/serialization.rs` (34 lines, 0 tests)
- `crates/core/src/state/mod.rs` (644 lines, 6 tests)
- `crates/core/src/state/storage.rs` (294 lines, 7 tests)
- `crates/core/src/utils.rs` (97 lines, 0 tests)

### R3 — core transaction queue (1,540 lines)

- `crates/core/src/tx/fees.rs` (109 lines, 3 tests)
- `crates/core/src/tx/mod.rs` (719 lines, 9 tests)
- `crates/core/src/tx/signer.rs` (118 lines, 1 tests)
- `crates/core/src/tx/storage.rs` (507 lines, 7 tests)
- `crates/core/src/tx/types.rs` (87 lines, 0 tests)

### R4 — validator DKG path (3,228 lines)

- `crates/validator/src/frost/mod.rs` (258 lines, 1 tests)
- `crates/validator/src/frost/keygen.rs` (516 lines, 0 tests)
- `crates/validator/src/frost/ecdh.rs` (181 lines, 4 tests)
- `crates/validator/src/frost/participants.rs` (33 lines, 1 tests)
- `crates/validator/src/frost/marshal.rs` (176 lines, 0 tests)
- `crates/validator/src/frost/error.rs` (46 lines, 0 tests)
- `crates/validator/src/state/keygen.rs` (1459 lines, 0 tests)
- `crates/validator/src/consensus/mod.rs` (5 lines, 0 tests)
- `crates/validator/src/consensus/group.rs` (459 lines, 5 tests)
- `crates/validator/src/consensus/epoch.rs` (95 lines, 1 tests)

### R5 — validator signing path and secrets (3,329 lines)

- `crates/validator/src/frost/preprocess.rs` (189 lines, 1 tests)
- `crates/validator/src/frost/sign.rs` (204 lines, 1 tests)
- `crates/validator/src/merkle.rs` (142 lines, 4 tests)
- `crates/validator/src/secrets/mod.rs` (6 lines, 0 tests)
- `crates/validator/src/secrets/nonces.rs` (348 lines, 3 tests)
- `crates/validator/src/secrets/store.rs` (969 lines, 13 tests)
- `crates/validator/src/state/preprocess.rs` (253 lines, 0 tests)
- `crates/validator/src/state/sign.rs` (868 lines, 0 tests)
- `crates/validator/src/state/transactions.rs` (101 lines, 0 tests)
- `crates/validator/src/consensus/hashing.rs` (249 lines, 4 tests)

### R6 — validator service, wiring, config (2,171 lines)

- `crates/validator/src/state/mod.rs` (516 lines, 0 tests)
- `crates/validator/src/service/action.rs` (381 lines, 0 tests)
- `crates/validator/src/service/effect.rs` (328 lines, 0 tests)
- `crates/validator/src/service/mod.rs` (129 lines, 0 tests)
- `crates/validator/src/bindings.rs` (247 lines, 0 tests)
- `crates/validator/src/config.rs` (290 lines, 4 tests)
- `crates/validator/src/main.rs` (99 lines, 0 tests)
- `crates/validator/src/metrics.rs` (181 lines, 0 tests)
- `crates/validator/validator.sample.toml`
- `crates/validator/Dockerfile`

### R7 — sentinel (whole crate) (3,872 lines)

- `crates/sentinel/src/action.rs` (43 lines, 0 tests)
- `crates/sentinel/src/bindings.rs` (173 lines, 0 tests)
- `crates/sentinel/src/config.rs` (144 lines, 4 tests)
- `crates/sentinel/src/effect.rs` (134 lines, 1 tests)
- `crates/sentinel/src/engine.rs` (392 lines, 10 tests)
- `crates/sentinel/src/hashing.rs` (224 lines, 5 tests)
- `crates/sentinel/src/main.rs` (89 lines, 0 tests)
- `crates/sentinel/src/metrics.rs` (143 lines, 0 tests)
- `crates/sentinel/src/service.rs` (2158 lines, 16 tests)
- `crates/sentinel/src/state.rs` (372 lines, 6 tests)
- `crates/sentinel/sentinel.sample.toml`
- `crates/sentinel/Dockerfile`

### R8 — cross-cutting (0 lines)

- `Cargo.toml`, `Cargo.lock`, `crates/core/Cargo.toml`, `crates/validator/Cargo.toml`, `crates/sentinel/Cargo.toml`, all three Dockerfiles and sample configs; sweeps over every file above

## Critics and QA (Phase 2/3 plan)

- Critics: `C2-CORE-A` (R1), `C2-CORE-B` (R2+R3), `C2-VAL-A` (R4), `C2-VAL-B` (R5+R6), `C2-SEN` (R7), `C2-XC` (R8), plus a Coverage Critic. Split further if any reviewer files more than ~25 findings.
- QA: one agent per crate holding a Confirmed or Plausible finding, plus one for cross-cutting; local Anvil ports assigned per agent (core/sentinel 8545–8549, validator 8645–8649, cross-cutting 8745–8749).
