# F-SEN-010 The sample config ships zero addresses that parse and start cleanly, and the pending "sensible default" decision keeps the zero-address failure mode alive

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | sentinel, config.rs / sentinel.sample.toml |
| Location | crates/sentinel/src/config.rs:41-59, 117-134, 136-143 (related: crates/sentinel/sentinel.sample.toml:23-39) |
| Severity | Informational / Informational |
| Certainty | 85% (set by Critic C-SEN; QA may raise) |
| Assumptions involved | A1 |
| Tags | config, known |

## Claim

The `TODO(epic Phase E2, follow-up)` at `config.rs:44-48` records that a default for `voting_window` is still to be chosen and that `fee_token`/`oracle`/`consensus` deliberately stay required "so a missing value fails loudly rather than silently using the wrong window/zero address". The guard the code actually implements is only against a _missing_ field: `deny_unknown_fields` plus a required `Address` catches an omitted key (tested at `config.rs:117-134`), but nothing rejects an address that is present and equal to `0x0000…0000`.

`sentinel.sample.toml` — the file the handbook tells operators to copy — ships exactly that: `oracle`, `consensus` and `[sentinel].fee_token` are all the zero address, and the only test over the sample asserts that it _parses_ (`config.rs:136-143`). An operator who copies it and fills in only some of the fields gets a binary that starts successfully, connects to the RPC, opens its database, serves metrics, indexes the zero address (from which no event ever arrives) and reports nothing wrong. The `consensus` case is the quietest: request ids are computed from the configured `consensus` address (`service.rs:108-115`), so a zero value makes every `NewRequest` mismatch and be dropped at `debug` level (`service.rs:286-289`) — see F-SEN-007.

The sample also ships a placeholder private key in the `signer` field. It is a well-known low-integer test key (not quoted here per the audit brief) whose address is public and unfunded, so copying the sample unchanged produces a sentinel that cannot transact rather than one that leaks anything — but it is a real key, not an obviously invalid placeholder, so a copy-paste mistake produces a running binary rather than a startup error.

This finding covers the `known` items recorded at `codebase-map.md` Section 4 for `crates/sentinel/src/config.rs:44` and `:121` (epic E2) and is filed at reduced priority accordingly.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The `TODO` states the intent — fail loudly rather than use a zero address — and defers the decision | E2 | crates/sentinel/src/config.rs:41-48 | `/// Configuration specific to the sentinel's request handling, as opposed to`<br>`/// the infrastructure it shares with other Safenet services.`<br>`//`<br>`// TODO(epic Phase E2, follow-up): pick and document a sensible default for`<br>`// \`voting_window\` (\`fee_token\`/\`oracle\`/\`consensus\` are deployment-specific and`<br>`// should stay required) once the sentinel's config shape has settled; for now`<br>`// it is mandatory so a missing value fails loudly rather than silently using`<br>`// the wrong window.` |
| 2 | The only enforcement is "the key must be present"; a present-but-zero address passes | E2 | crates/sentinel/src/config.rs:117-128 | `    #[test]`<br>`    fn rejects_config_missing_a_deployment_specific_field {`<br>`        // \`oracle\`, \`consensus\` and the \`[sentinel]\` block have no sensible`<br>` // default and must fail loudly rather than silently defaulting to the`<br>` // zero address (see the \`SentinelConfig\` TODO above).`<br>` let without_oracle = TOML.replacen(`<br>` r#"oracle = "0x0101010101010101010101010101010101010101""#,`<br>` "",`<br>` 1,`<br>` );`<br>` assert!(toml::from_str::<Config>(&without_oracle).is_err);`<br>` }` |
| 3 | `Config::load` performs no validation beyond TOML deserialisation | E2 | crates/sentinel/src/config.rs:61-67 | `impl Config {`<br>`    pub async fn load(file: &Path) -> Result<Self, Error> {`<br>`        let contents = fs::read_to_string(file).await?;`<br>`        let config = toml::from_str(&contents)?;`<br>`        Ok(config)`<br>`    }`<br>`}` |
| 4 | The sample ships three zero addresses | E2 | crates/sentinel/sentinel.sample.toml:23-31 | `# The \`SentinelOracle\` contract watched and voted/committed on.`<br>`oracle = "0x0000000000000000000000000000000000000000"`<br>``<br>`# The \`Consensus\` contract whose proposals are hashed into request ids.`<br>`consensus = "0x0000000000000000000000000000000000000000"`<br>``<br>`[sentinel]`<br>`# The ERC-20 fee token approved for bonds.`<br>`fee_token = "0x0000000000000000000000000000000000000000"` |
| 5 | The only assertion made about the sample is that it parses | E2 | crates/sentinel/src/config.rs:136-143 | `    #[test]`<br>`    fn parses_sample_config {`<br>`        // The sample linked from the sentinel handbook must stay a valid,`<br>`        // loadable example of the schema above.`<br>`        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("sentinel.sample.toml");`<br>`        let contents = std::fs::read_to_string(path).unwrap;`<br>`        toml::from_str::<Config>(&contents).unwrap;`<br>`    }` |
| 6 | Startup consumes the values with no check of any kind | E2 | crates/sentinel/src/main.rs:75-83 | `    let driver = Driver::new(`<br>`        service,`<br>`        provider,`<br>`        config.signer,`<br>`        pool,`<br>`        vec![config.oracle, config.consensus],`<br>`        config.driver,`<br>`    )`<br>`    .await?;` |

## Trigger

1. An operator follows `sentinel.sample.toml:1-7` ("Copy this file, fill in the deployment-specific values below") and fills in `rpc`, `signer`, `database` and `[sentinel].engine`, but misses one of the three address fields — the `[sentinel]` block is visually separated from the top-level `oracle`/`consensus`, so `fee_token` is the most likely to be missed.
2. `Config::load` succeeds (basis 3); `main` starts, connects, and the watcher subscribes to `[oracle, consensus]` (basis 6). With a zero `oracle` or `consensus`, no event ever arrives and the process looks healthy — logs are quiet, metrics are served, `safenet_core_block_number` advances normally.
3. With a zero `fee_token` only, the sentinel does everything right up to `ApproveToken`, which is sent to `0x0` — an EOA-like address with no code, so the call succeeds trivially and the following `commit` reverts inside `safeTransferFrom`. This is F-SEN-007's failure mode, reached from a copy-paste rather than from a wrong address.

## Considered and rejected

- **"`deny_unknown_fields` protects the config."** It protects against typo'd _keys_ (`config.rs:18, 50`), which is genuinely useful, but says nothing about values.
- **"The zero address would obviously fail."** It fails silently, not obviously — see the trigger, and F-SEN-007 basis 6 for the `debug`-level log that is the only signal.
- **"Placeholder keys should be reported as a secret leak."** They should not: the audit brief records sample keys as placeholders, and this one is unfunded and public. The concern here is that it is a _valid_ key, so the binary starts instead of refusing.
- **"This duplicates F-SEN-007."** F-SEN-007 is about the absence of onchain cross-validation for _any_ value; this one is about the shipped sample and the pending default decision specifically, and is the file that carries the `known` tag for `config.rs:44`/`:121`. Remediation option 1 below is the narrow fix; F-SEN-007 option 1 is the broad one.
- **False positive check — does `alloy`'s `Address` deserialiser reject the zero address?** No; `Address` is a plain 20-byte value and `Address::ZERO` is used as a legitimate value throughout the codebase (for example `gasToken: Address::ZERO` at `crates/sentinel/src/hashing.rs:124` and `refundReceiver: Address::ZERO` at `:125`).

## Remediation options

1. **Reject zero addresses at load time.** Add a `Config::validate` (or `#[serde(deserialize_with = ...)]` on the three fields) rejecting `Address::ZERO` for `oracle`, `consensus` and `fee_token`, and a `voting_window` floor (F-SEN-009 option 2). Two lines each, and it turns the copy-paste failure into a startup error.
2. **Make the sample obviously incomplete.** Replace the zero addresses with a clearly invalid marker (`"0xFILL_ME_IN"`), which fails to deserialise, and change `parses_sample_config` into a test asserting the sample does **not** load as-is while a version with the placeholders substituted does. Tradeoff: the sample is no longer directly loadable, which is the point.
3. **Resolve the `TODO`.** Decide the `voting_window` default (or remove the field entirely in favour of reading `COMMIT_WINDOW` from the oracle, F-SEN-009 option 1) and delete the comment, so the deferred decision stops standing in for validation.
4. Document in the handbook that the sample's `signer` is a public test key and must be replaced.

Tests to add: `rejects_config_with_a_zero_address` covering each of the three fields; a sample-config test that asserts the shipped file is rejected as-is.

## Trail

- Reviewer R7: drafted to cover the `known` items at `config.rs:44` and `:121` (codebase-map Section 4, epic E2), self-estimate 85%. All six basis citations re-opened in this checkout. Tagged `known` and filed at Informational per A12.

## Critic (C-SEN)

### Per-claim verdicts

All rows re-opened (`config.rs:41-48`, `:61-67`, `:117-128`, `:136-143`, `sentinel.sample.toml:23-31`, `main.rs:75-83`). Every quote is accurate; no claim marked `H`. Confirmed by reading both files in full: the only assertion over the sample is `parses_sample_config`, which calls `toml::from_str::<Config>(&contents).unwrap` and checks nothing else, and `rejects_config_missing_a_deployment_specific_field` removes a key rather than zeroing a value — so the "fails loudly" property the `TODO` claims is only tested for _absence_, not for the zero address the sample actually ships.

I checked the one thing that would change the severity and it does not: the placeholder `signer` value is a well-known low-integer test key whose address is public and unfunded, so copying the sample unchanged yields a sentinel that cannot transact rather than one that leaks anything. Under A1 the operator provisions this file, so nothing here is a secret-handling finding.

Worth adding to the report as a one-line note: `main.rs:80` passes `vec![config.oracle, config.consensus]` to the watcher, so with the sample's zero addresses the process indexes `0x0` and reports healthy forever — the concrete shape of "starts cleanly and does nothing".

### Finding verdict

**Confirmed. Certainty 85%. Severity Informational (unchanged).**

Mechanism and trigger are both trivially `E2` (the file is in the repository and the test asserts only that it parses). Informational is correct per Section 8 — a hardening and test-gap item, correctly tagged `known` (A12) against the E2-epic `TODO`s at `config.rs:44` and `:121`.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

**Note for every sentinel finding whose "tests to add" list names a `service.rs` unit test:** `sentinel` is a **binary-only crate** — `crates/sentinel/src/main.rs` declares `mod service;` and there is no `lib.rs`, so the crate has no library target and `crates/sentinel/tests/` cannot compile against it. Every such test must live inside the existing `#[cfg(test)] mod tests` in the source file. If the team wants these as permanent regression tests reachable from an integration target, **the crate needs a `lib.rs` first**; that is an unstated prerequisite across F-SEN-001, -002, -003, -011, -012 and -015.

### Remediation check

**Sound: option 1 plus option 2, and they belong in one change.**

Option 1 (reject `Address::ZERO` for `oracle`, `consensus` and `fee_token`, plus a `voting_window` floor) is sound and is the same `Config::validate` pass **F-SEN-009 option 2** wants. One function, two findings.

Option 2 (replace the sample's zero addresses with a marker that fails to deserialise, and change `parses_sample_config` into a test asserting the sample does **not** load as-is) is sound and is the half that actually prevents the copy-paste failure. Its stated tradeoff — the sample is no longer directly loadable — is the point, and the existing test that asserts it _does_ load is currently pinning the hazard as if it were a feature. Both halves are needed: validation without the sample change leaves a shipped file that fails at startup with a confusing error; the sample change without validation leaves a hand-edited config free to reintroduce a zero address.

Option 3 (resolve the `voting_window` `TODO`, or remove the field in favour of reading `COMMIT_WINDOW` — F-SEN-009 option 1) is sound and is worth doing so that a deferred decision stops standing in for validation.

Option 4 (document that the sample's `signer` is a public test key) is necessary and is the kind of line that gets skipped; under **A1** the key file is operator-readable by design, so the risk here is not disclosure but an operator shipping the _sample_ key to production, which is a different and worse failure.

## Post-merge revalidation (RV-SEN)

Re-validated against merge commit `a7f3915` (baseline `2893917`).

### Verdict: **STILL VALID** — neither file changed

`crates/sentinel/src/config.rs` and `crates/sentinel/sentinel.sample.toml` are both untouched by the merge (`git diff 2893917 HEAD -- crates/sentinel/src/config.rs crates/sentinel/sentinel.sample.toml` is empty), so every citation stands verbatim: the `TODO(epic Phase E2, follow-up)` at `config.rs:44-48`, the parse-only tests at `config.rs:117-134` and `:136-143`, and the zero addresses plus the placeholder private key at `sentinel.sample.toml:23-39`.

The one `service.rs` citation is byte-identical: the request-id derivation from the configured `consensus` address at `service.rs:108-115`, and the `debug`-level drop of a mismatched `NewRequest` at `service.rs:286-289`.

**Certainty 85% and severity Informational unchanged.** Status left at `Critiqued`; the `known` tag still applies.
