# Dependency questions settled by execution — V-CORE-SEN, Phase 5

Answers to questions in `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` that V-CORE-SEN closed with a toolchain (cargo 1.98.1 / rustc 1.98.1, `stable-aarch64-unknown-linux-gnu`, commit `2893917`). Written as a separate file rather than appended to the questions file, because that file was already lost once to a concurrent write (`rust-audit/state/STATE.md`, "Data loss and recovery") and other Phase 5 agents were running while this was written.

---

## Question 2 — does `alloy-sol-types` 1.6.0 reject invalid UTF-8 in a `string` field?

**Answer: NO. It decodes lossily, returning `Ok` with U+FFFD. Basis 8 of F-SEN-013 is REFUTED.**

- Source: `alloy-sol-types-1.6.0/src/types/data_type.rs:368-388` — `impl SolType for String` has a checked `valid_token` (`core::str::from_utf8(...).is_ok`) but a **lossy** `detokenize` (`RustString::from_utf8_lossy`), with an explicit comment saying lossy is deliberate.
- `valid_token` is reached only through the `*_validate` family: `src/types/event/mod.rs:185-211` shows `decode_raw_log` → `abi_decode_data` → `abi_decode_sequence` (no validation), while `decode_raw_log_validate` → `abi_decode_data_validate`.
- `watcher_events!` generates the **non-validating** `SolEventInterface::decode_raw_log` (`crates/core/src/index/events.rs:546-554`), so this codebase takes the lossy path.
- Executed through that exact path: `SentinelEvents::decode_log(topics, data)` for a `Revealed` whose `reason` is the single byte `0x80` returns `Some(...)` with `reason: "\u{fffd}"`. Same for `DisputeResolved.context` and `DisputeOutOfScope.context`.
- Artifacts: `rust-audit/poc/Q2-utf8/`.

Consequences: **F-SEN-013 → Informational** (severity resolved; see its `## Verification` section). **F-CORE-004 is NOT closed** — the batch-poisoning mechanism is untouched; only the cheap attacker-chosen path into it is gone.

---

## Question 11 — what does `estimate_eip1559_fees` issue and return when `reward` is empty?

**Answer: one `eth_feeHistory(0x0a, "latest", [20.0])`, and a priority fee floored at 1 wei.**

- Issued request: `alloy-provider-2.0.5/src/provider/trait.rs:276-306` calls `get_fee_history(EIP1559_FEE_ESTIMATION_PAST_BLOCKS = 10, Latest, &[EIP1559_FEE_ESTIMATION_REWARD_PERCENTILE = 20.0])` → `eth_feeHistory` with `(U64(10), "latest", [20.0])`. If the latest block's base fee is absent or zero it additionally issues `eth_getBlockByNumber("latest")` and errors `UnsupportedFeature("eip1559")` if that block has no base fee.
- Returned value: `reward.unwrap_or_default` is passed to `eip1559_default_estimator` (`alloy-provider-2.0.5/src/utils.rs:93-125`). `estimate_priority_fee` filters out zero rewards and returns `EIP1559_MIN_PRIORITY_FEE = 1` when nothing is left; `max_fee = base_fee * EIP1559_BASE_FEE_MULTIPLIER(2) + priority`.
- Executed against a mocked provider (no anvil on this host; the estimator is pure in `(base_fee, rewards)`, so the mock is equivalent). For `base_fee_per_gas: [100, 100]`, all three empty shapes give the same answer:

  ```
  reward = Some([])       -> Eip1559Estimation { max_fee_per_gas: 201, max_priority_fee_per_gas: 1 }
  reward = None           -> Eip1559Estimation { max_fee_per_gas: 201, max_priority_fee_per_gas: 1 }
  reward = [[0],[0]]      -> Eip1559Estimation { max_fee_per_gas: 201, max_priority_fee_per_gas: 1 }
  ```

- Artifacts: `rust-audit/poc/Q11-fee-estimator/`.

Consequence for **F-CORE-060**: the real starting level is **conservative** (1 wei priority), so the absolute wei figures in its part 1 (which start from the mock's 10/210) over-state the money by roughly an order of magnitude. The finding is **not** softened: the ratchet compounds off the previous submission, never off the estimate, so a 1-wei honest floor widens the gap between the honest estimate and the ratcheted fee. Quote part 1's numbers as a mock, not as production wei.

---

## Question 10 — does axum's default `JsonRejection` echo a fragment of the request body?

**Answer: YES — it echoes the offending JSON _field name_ (path) verbatim in a `422` plain-text body. It does not echo field _values_.**

Run against the real binary: `./target/debug/sentinel-engine --config-file <cfg>` with `rpc` pointed at a 40-line local stub answering `eth_chainId` (the engine calls `Provider::connect` at `main.rs:51`, **before** it binds its listener at `:75`, so it will not start without a reachable RPC — worth knowing on its own). Config and stub saved in `rust-audit/poc/Q10-axum-rejections/`; full transcript in that directory's `RESULT-V-CORE-SEN.out`.

| Probe | Status | Body |
| --- | --- | --- |
| `POST /v1/security-check`, `content-type: application/json`, `{"chainId": "not-a-number", "safe": "0x00"}` | `422 Unprocessable Entity` | `Failed to deserialize the JSON body into the target type: chainId: unknown field `chainId`, expected `block`or`transaction` at line 1 column 10` |
| `POST /v1/security-check`, no content-type, `not json at all` | `415 Unsupported Media Type` | `Expected request with `Content-Type: application/json`` |
| `GET /v1/security-check` | `405 Method Not Allowed` | empty, with `allow: POST` |
| `POST /v1/nonexistent` | `404 Not Found` | empty |
| `POST /v1/security-check`, `content-type: text/plain`, `{}` | `415 Unsupported Media Type` | `Expected request with `Content-Type: application/json`` |
| `POST /v1/security-check`, `content-type: application/json`, `{"totallyUnknownField": "SECRET-VALUE-12345"}` | `422 Unprocessable Entity` | `Failed to deserialize the JSON body into the target type: totallyUnknownField: unknown field `totallyUnknownField`, expected `block`or`transaction` at line 1 column 22` |

The last probe is the decisive one: the attacker-chosen **key** `totallyUnknownField` is reflected twice, while its value `SECRET-VALUE-12345` is not. All 422 bodies also disclose the expected schema (`expected `block`or`transaction``) and a line/column offset into the submitted body.

Consequence for **F-ENG-008**: the statuses the service can actually return are `200`, `422`, `415`, `405`, `404` plus the handler's own `(StatusCode, &'static str)` pairs (`crates/sentinel-engine/src/api/extractors.rs:22-25`, `:43-46`), and `docs/sentinel-engine.md` documents only a subset. Under A3 the caller is the trusted co-deployed sentinel, so the reflected key name is Informational; the undocumented status set is the substance. **Not tested:** whether a panicking handler yields no response at all (`tower-http` is taken with `["trace"]` only, no `catch-panic`) — that leg of the question is still open.
