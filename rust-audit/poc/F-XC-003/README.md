# PoC — F-XC-003 (`deny_unknown_fields` + `#[serde(flatten)]`)

**Never compiled, never run.** No Rust toolchain on the audit machine (A9 FALSE). Written against the checkout at commit `2893917`.

## What this settles

Whether a mistyped key in `validator.toml` or `sentinel.toml` is **rejected** or **silently accepted**. Serde's behaviour for `#[serde(deny_unknown_fields)]` on a container that also has a `#[serde(flatten)]` field is a known library subtlety and its source is not on disk (A6), so `F-XC-003`'s pivotal leg is class `I` and the finding sits at 58% (Plausible).

There is a fact in this checkout that survives whatever serde does, and it is why the question matters: the container everything flattened is routed _into_, `core::driver::Config`, is `#[serde(default)]` with **no** `deny_unknown_fields` (`crates/core/src/driver.rs:29-37`) — unlike all five of its sibling config structs in `core` (`tx/mod.rs:70`, `index/blocks.rs:48`, `index/mod.rs:21`, `index/events.rs:72`, `observability/mod.rs:17`). An unknown key that reaches it is dropped in silence.

The engine crate has `rejects_unknown_field` (`crates/sentinel-engine/src/config.rs:129-144`) and is also the one config with **no** flattened field, so its green test says nothing about the interaction. The validator and the sentinel — the two that do flatten — have no such test.

## Install and run

| File | Paste into | Command |
| --- | --- | --- |
| `append-to-crates-validator-src-config.rs` | the existing `mod tests` block at the end of `crates/validator/src/config.rs` | `cargo test -p validator config::tests::qa_xc_003` |
| `append-to-crates-sentinel-src-config.rs` | the existing `mod tests` block at the end of `crates/sentinel/src/config.rs` (it reuses that module's `TOML` constant at `:74-84`) | `cargo test -p sentinel config::tests::qa_xc_003` |

Both are unit tests inside binary-only crates, so they cannot be integration tests.

## Reading the result

| Outcome | Meaning | Action |
| --- | --- | --- |
| All six **pass** | `deny_unknown_fields` is honoured despite the flatten. | `F-XC-003`'s `I` leg closes; the finding drops to **Informational** (a test gap that is now filled). Keep the tests — they are the regression guard, and they cost nothing. |
| `qa_xc_003_rejects_an_unknown_top_level_key` **fails** (either crate) | A stray top-level key is swallowed. | `F-XC-003` becomes **Confirmed**; severity **Low → Medium** is defensible because the failure is undetectable at runtime: `crates/validator/src/main.rs:43` logs only the config file path, there is no startup echo of the resolved configuration and no metric. This also confirms the same leg in `F-VAL-063`. |
| `qa_xc_003_rejects_a_typo_in_a_security_relevant_key` **fails** | A typo in `[index] use_client_filtering` is swallowed. | The sharpest form: the validator runs with node-filtered, unverified `eth_getLogs` results (A4, `CORE-H2`) while the operator believes client-side verification is on. Same escalation, and it should be called out by name in the report. |
| `qa_xc_003_rejects_a_typo_inside_the_sentinel_table` **fails** | The control case failed. | Something larger is wrong: `SentinelConfig` has `deny_unknown_fields` and no flattened field (`crates/sentinel/src/config.rs:48-58`), so this must pass. Investigate the fixture before trusting the other results. |
| The two crates **disagree** | The validator flattens with `#[serde(default, flatten)]`; the sentinel with a bare `#[serde(flatten)]`. | That difference is itself the answer, and the remediation must handle both forms. |

## Fixtures

Attacker input is not involved — this is an honest operator's typo under A1. The literal inputs are in the test bodies: `not_a_real_field = "typo"` at top level, `use_client_filterring` (double `r`) inside `[index]`, `max_reorg_dept` (missing `h`) inside `[index]`, `votin_window` inside `[sentinel]`, and `[observabilty]` as a mistyped table name.

## Remediation check (QA-XC)

`F-XC-003` remediation option 1 is "add these tests", so this PoC **is** the remediation and it is sound as far as it goes — but it is a detector, not a fix. If the tests fail, the fix is one of:

1. Add `#[serde(deny_unknown_fields)]` to `core::driver::Config` (`crates/core/src/driver.rs:30`), matching its five siblings. **This is the one I would take** — it is one line, it makes the repo internally consistent, and it closes the half of the defect that does not depend on serde's behaviour at all. Note that it does _not_ on its own guarantee the outer container's attribute starts working; the tests above are how you find out.
2. Drop the flatten and give `driver` an explicit `[driver]` table. Correct but a breaking configuration change for every deployed operator, and the handbooks would need rewriting.
3. Echo the fully resolved configuration at `info` on startup. This does not prevent the typo but converts a silent degradation into something an operator can see, and it is the same remediation `F-ENG-009` asks for in the engine. Cheap, and worth doing regardless of the outcome above.

No documented runtime contract is affected (`core::state`'s purity, effect-replay and resume-ordering rules are not involved), and there is no Solidity reference for configuration.
