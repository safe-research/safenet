# F-VAL-063 Consensus-critical configuration is unvalidated, has no onchain anchor, and its defaults are the unsafe ones

| Field | Value |
| --- | --- |
| Status | QA-done |
| Crate and module | validator, config.rs (+ validator.sample.toml) |
| Location | crates/validator/src/config.rs:53-104 (related: crates/validator/validator.sample.toml:23-56, crates/validator/src/service/mod.rs:49-59, crates/validator/src/state/transactions.rs:32-40, crates/validator/src/consensus/group.rs:282-297, crates/validator/src/state/keygen.rs:514-526) |
| Severity | Medium / Medium |
| Certainty | 72% (Critic C-VAL-B; QA may raise) |
| Assumptions involved | A1, A10 |
| Tags | config, input-validation |

## Claim

`ValidatorConfig` carries four values that must be identical across the whole validator set for consensus to work — `participants`, `blocks_per_epoch`, `genesis_salt`, and (for the transaction path) `oracles` — and none of them has an onchain source, a cross-validation step, or a startup consistency check. The only validation performed anywhere is `ValidatorService::new`'s "can a genesis group be formed at all" test. A single operator's typo therefore produces a validator that runs, reports healthy, emits normal metrics, and is silently absent from consensus.

Three specific problems, in decreasing severity:

1. **`blocks_per_epoch` is a consensus parameter that lives only in local TOML.** It multiplies into `rollover_block`, which is bound into the EIP-712 `EpochRollover` message the group must jointly sign, and into the `stageEpoch` calldata. `Consensus._requireValidRollover` checks only `epochs.active < proposedEpoch && rolloverBlock > block.number && epochs.staged == 0` — it has no notion of an epoch length, so it cannot reject a divergent value. A validator with a different `blocks_per_epoch` computes a different message hash, never joins the rollover signature, and additionally advances its own `active_epoch` at the wrong block. Nothing detects the divergence; the validator just stops attesting.

2. **`genesis_salt` defaults to zero, and zero is exactly the value that disables what the salt is for.** `genesis_context` short-circuits to `B256::ZERO` for a zero salt, so the genesis group id is a function of the participant set alone. The doc comment two lines above states the salt exists so that "the same validator set [can] work for multiple consensus contracts without needing to rotate the validator accounts" — the default therefore silently opts out of deployment separation, and `validator.sample.toml` ships the zero salt as an explicit, uncommented setting. Two deployments (a redeploy, a testnet fork, a staging environment) sharing a participant list share a genesis group id, and the reorg-immune `SecretStore` is keyed on that group id.

3. **An empty `oracles` set is the default and means "never attest anything".** `handle_transaction_proposed` drops every proposal from an unlisted oracle at `debug` level; with the set empty, that is every proposal. The sample config ships the field commented out. A validator configured from the sample is a fully functional DKG participant that will never attest a single transaction, and the only evidence is a `debug` log the default `log_filter = "info"` suppresses.

Two supporting gaps: nothing binds the SQLite database to a chain id or contract set (the snapshot table is `(block_number, state)` and neither `SecretStore` nor the transaction queue records one), so repointing `rpc`/`consensus` at a different deployment resumes an unrelated snapshot by block number; and the four `NonZeroU64` timeouts are validated only for non-zero-ness, so `key_gen_timeout * 3 > blocks_per_epoch` (a DKG that cannot finish inside an epoch) or `oracle_timeout < signing_timeout` are accepted without comment.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `oracles` and `participants` default to empty and there is no validation beyond serde | E2 | `crates/validator/src/config.rs:62-72` | `    /// The validator set participating in key generation and signing, each with`<br>`    /// the epoch window during which it is active.`<br>`    #[serde(default)]`<br>`    pub participants: Vec<Participant>,`<br>`    /// The oracle contracts whose results the validator honors when signing`<br>`    /// oracle transactions.`<br>`    #[serde(default)]`<br>`    pub oracles: BTreeSet<Address>,`<br>`    /// The salt mixed into the genesis group's key generation context.`<br>`    #[serde(default)]`<br>`    pub genesis_salt: B256,` |
| 2 | An empty `oracles` set makes the validator drop every transaction proposal, logged only at `debug` | E2 | `crates/validator/src/state/transactions.rs:32-40` | `        if !self.config.oracles.contains(&event.oracle) {`<br>`            tracing::debug!(`<br>`                ?epoch,`<br>`                oracle = %event.oracle,`<br>`                safe_tx_hash = %event.safeTxHash,`<br>`                "ignoring transaction proposal from unknown oracle"`<br>`            );`<br>`            return (state, Vec::new);`<br>`        }` |
| 3 | The sample config ships `oracles` commented out, i.e. at that default | E2 | `crates/validator/validator.sample.toml:34-36` | `# Optional: oracle contracts whose results this validator honors when`<br>`# attesting oracle-checked transactions.`<br>`# oracles = ["0x0000000000000000000000000000000000000000"]` |
| 4 | The only startup-time validation is that a genesis group can be formed; nothing else is checked | E2 | `crates/validator/src/service/mod.rs:49-59` | `    ) -> Result<Self, Error> {`<br>`        let secrets = SecretStore::new(pool).await?;`<br>`        let genesis = group::participants_set(`<br>`            &config.participants,`<br>`            Epoch::Genesis {`<br>`                salt: config.genesis_salt,`<br>`            },`<br>`        )`<br>`        .ok_or(Error::InvalidValidators)?;`<br>`        let consensus = ConsensusDomain::new(chain_id, config.consensus);`<br>`` |
| 5 | `blocks_per_epoch` determines `rollover_block`, which is bound into the EIP-712 rollover message every validator must agree on | E2 | `crates/validator/src/state/keygen.rs:514-526` | `                        // Compute the rollover package that needs to be`<br>`                        // attested for the epoch to get staged.`<br>`                        let active_epoch = state.active_epoch;`<br>`                        let group_key = participation.group_commitments.group_key;`<br>`                        let rollover_block = proposed_epoch`<br>`                            .get`<br>`                            .saturating_mul(self.config.blocks_per_epoch.get);`<br>`                        let message = self.consensus.epoch_rollover_hash(`<br>`                            active_epoch,`<br>`                            proposed_epoch,`<br>`                            rollover_block,`<br>`                            &group_key,`<br>`                        );` |
| 6 | and it also drives the local rollover clock and the epoch the validator considers active | E2 | `crates/validator/src/consensus/epoch.rs:4-9` | `/// Returns the next epoch number for `block`, given the configured number of`<br>`/// blocks per epoch.`<br>`pub const fn next_number(block: u64, blocks_per_epoch: NonZeroU64) -> NonZeroU64 {`<br>`    let number = block / blocks_per_epoch.get;`<br>`    NonZeroU64::MIN.saturating_add(number)`<br>`}` |
| 7 | The contract has no notion of an epoch length, so nothing onchain can reject a divergent value | E2 | `contracts/src/Consensus.sol:395-397` | `    function _requireValidRollover(Epochs memory epochs, uint64 proposedEpoch, uint64 rolloverBlock) private view {`<br>`        require(epochs.active < proposedEpoch && rolloverBlock > block.number && epochs.staged == 0, InvalidRollover);`<br>`    }` |
| 8 | `genesis_salt` defaults to zero, and zero collapses the genesis context to the zero hash | E2 | `crates/validator/src/consensus/group.rs:288-297` | `fn genesis_context(salt: B256) -> B256 {`<br>`    if salt == B256::ZERO {`<br>`        return B256::ZERO;`<br>`    }`<br>`   //`encodePacked(string "genesis", bytes32 salt)`.`<br>`    let mut buffer = [0u8; 7 + 32];`<br>`    buffer[..7].copy_from_slice(b"genesis");`<br>`    buffer[7..].copy_from_slice(salt.as_slice);`<br>`    keccak256(buffer)`<br>`}` |
| 9 | which is precisely the deployment separation the salt exists to provide | E2 | `crates/validator/src/consensus/group.rs:282-287` | `/// Genesis uses a different group context since we don't know the consensus`<br>`/// contract address as it depends on the genesis group ID (🐓 and 🥚 problem).`<br>`/// Instead, compute a different context based on the genesis salt (allowing the`<br>`/// genesis group ID to be parameterized and the same validator set to work`<br>`/// for multiple consensus contracts without needing to rotate the validator`<br>`/// accounts).` |
| 10 | and the sample config ships the zero salt explicitly, with no note about the consequence | E2 | `crates/validator/validator.sample.toml:38-39` | `# The salt mixed into the genesis group's key generation context.`<br>`genesis_salt = "0x0000000000000000000000000000000000000000000000000000000000000000"` |
| 11 | Nothing binds the database to a chain id or a contract set: the snapshot table is keyed by block number alone | E2 | `crates/core/src/state/storage.rs:50-55` | `        sqlx::query(`<br>`            "CREATE TABLE IF NOT EXISTS snapshots (`<br>`                 block_number INTEGER PRIMARY KEY,`<br>`                 state        TEXT    NOT NULL`<br>`             )",`<br>`        )` |
| 12 | The four timeout parameters are independently defaulted with no relational validation | E2 | `crates/validator/src/config.rs:89-103` | `    const fn default_blocks_per_epoch -> NonZeroU64 {`<br>`        NonZeroU64::new(1440).unwrap`<br>`    }`<br>``<br>`    const fn default_keygen_timeout -> NonZeroU64 {`<br>`        NonZeroU64::new(120).unwrap`<br>`    }`<br>``<br>`    const fn default_signing_timeout -> NonZeroU64 {`<br>`        NonZeroU64::new(6).unwrap`<br>`    }`<br>``<br>`    const fn default_oracle_timeout -> NonZeroU64 {`<br>`        NonZeroU64::new(12).unwrap`<br>`    }` |

## Trigger

No attacker is required; the trigger is ordinary operator error, which A1 does not exclude (A1 says the operator is honest, not infallible).

- Copy `validator.sample.toml`, fill in `rpc`, `signer`, `database`, `consensus` and the participant list, and leave `oracles` commented out as shipped. The validator completes genesis DKG, signs epoch rollovers, and silently ignores 100% of transaction proposals.
- Set `blocks_per_epoch = 1200` on one node while the rest of the set uses the 1440 default. That node computes `rollover_block = proposed_epoch * 1200`, produces a different `epoch_rollover_hash`, and never contributes a share to any rollover signature. If enough nodes drift, no rollover ever reaches threshold and the epoch chain stalls.
- Redeploy `Consensus` (new address, same validator accounts, default zero salt) while reusing `database`. The genesis group id is unchanged, so `store_keygen_secrets` returns the previous deployment's row instead of the freshly sampled one, and the state machine resumes a snapshot indexed by block number from the previous chain view.

## Considered and rejected

- **"`deny_unknown_fields` catches typos."** It catches misspelled _keys_, not wrong _values_, and none of the three problems above involves an unknown key. Separately, `Config` combines `#[serde(deny_unknown_fields)]` with `#[serde(default, flatten)] pub driver: driver::Config` (`config.rs:20-21`, `:36-38`); serde documents that combination as unsupported. I could not determine offline whether serde 1.x rejects it at derive time (in which case the attribute is honoured, since CI compiles) or silently disables it for the outer struct — there is no toolchain and no vendored serde source here, so this stays an open question rather than a claim. No test covers it: the four `config.rs` tests only assert successful parses.
- **"Duplicate `participants` entries would corrupt `count`."** Refuted: `participants_set` collects addresses into a `BTreeSet` before `count` is taken (`consensus/group.rs:180-184`, `:193-200`, `:206-211`), so duplicates dedupe harmlessly.
- **"A zero `consensus` address would be accepted."** It parses (a `config.rs` test uses `0x00..00`), but startup then fails loudly at `Consensus::getCoordinator` (`main.rs:49-52`) because there is no contract to call. Not a finding.
- **"`participants` empty is the dangerous default."** Refuted: `min_participants` has a floor of 2 (`consensus/group.rs:239`), so an empty or single-entry list makes `participants_set` return `None` and `ValidatorService::new` fail with `InvalidValidators`. That default fails loudly, which is why only `oracles` is called out above.
- **"`EpochRolledOver` should be watched so the local clock cannot drift."** Checked: `IConsensus` declares `event EpochRolledOver(uint64 indexed newActiveEpoch)` (`contracts/src/interfaces/IConsensus.sol:66`) and `bindings.rs` does not bind it. This is _not_ a divergence in itself, because the contract's own rollover is the same block-clock computation applied lazily (`Consensus.sol:378-386`), so the two agree whenever `blocks_per_epoch` agrees. Watching the event would, however, be the cheapest possible detector for problem 1, which is why it appears under remediation rather than as a separate finding.

## Remediation options

1. Validate at startup and refuse to run on an obviously broken combination: `oracles` empty (or at minimum a `warn!` naming the consequence), `key_gen_timeout * 3 >= blocks_per_epoch`, `oracle_timeout < signing_timeout`, `consensus` or `coordinator` appearing in `oracles`. All are pure functions of the parsed config and belong in a `ValidatorConfig::validate` called from `ValidatorService::new`, next to the existing `InvalidValidators` check.
2. Anchor the shared parameters onchain. `blocks_per_epoch` is the important one: either read it from `Consensus` (needs a contract change) or, cheaply and without one, subscribe to `EpochStaged`/`EpochRolledOver` and `error!` when the observed `rolloverBlock` or active epoch disagrees with the local computation. That turns silent exclusion into an alertable event.
3. Bind the database to its deployment. Add a one-row `deployment(chain_id, consensus, coordinator)` table written on first use and checked on every open; refuse to start on a mismatch. This is cheap, and it also closes CORE-H17's "no chain-id binding" for the sentinel.
4. Change the `genesis_salt` default to "required" (no `#[serde(default)]`) so an operator must make a deliberate choice, and update `validator.sample.toml` to explain that zero means "no deployment separation" rather than presenting it as a neutral placeholder.
5. Make the sample config represent a _working_ deployment rather than a minimal one: uncomment `oracles` with a placeholder and a note that an empty list disables all attestation.

Tests to add: a `config.rs` test asserting `toml::from_str::<Config>` rejects an unknown top-level key (settles the `flatten` question and guards it); a test asserting `ValidatorConfig::validate` rejects each dangerous combination; and an assertion in `parses_sample_config` that the parsed sample would pass `validate`, so the sample cannot drift away from a runnable configuration.

## Trail

- Reviewer R6: drafted from validator checklist item 8 ("Config trust") and the cross-cutting checklist item 8. Every cited line re-opened in this checkout, including the Solidity side of the `blocks_per_epoch` claim. Self-estimate 80% overall: the three problems are each directly readable from the cited code, and the residual doubt is about severity framing rather than mechanism — how much of this the team already handles in deployment tooling outside the repo is not visible from here.

## Critic (C-VAL-B)

Derived from `config.rs` in full, `validator.sample.toml` in full, `service/mod.rs:41-69`, `consensus/group.rs:177-297`, `state/transactions.rs:16-40` and `contracts/src/Consensus.sol:395-397` before reading the Claim.

### Per-claim verdicts

All basis rows **Supported**; every citation re-opened and matched. The three specifics check out exactly:

1. `blocks_per_epoch` has no onchain anchor. I re-read `_requireValidRollover`: `require(epochs.active < proposedEpoch && rolloverBlock > block.number && epochs.staged == 0, InvalidRollover)` (`contracts/src/Consensus.sol:395-397`) — no notion of epoch length, so the contract genuinely cannot reject a divergent value, and `rollover_block` is `proposed_epoch * blocks_per_epoch` bound into the signed `EpochRollover` message (`state/keygen.rs:518-526`). A node with a different value produces a different message hash and simply never joins the signature. Supported.
2. `genesis_context` short-circuits to `B256::ZERO` for a zero salt (`consensus/group.rs:288-291`), the default is `B256::default` (`config.rs:70-72`), and `validator.sample.toml:38-39` ships the zero salt as an _uncommented, explicit_ setting — I confirm the sample makes the unsafe value look deliberate. Supported.
3. `oracles` defaults to an empty `BTreeSet` (`config.rs:67-69`), `validator.sample.toml:36` ships it commented out, and `handle_transaction_proposed` drops every unlisted oracle at `debug` (`state/transactions.rs:32-40`) which the default `log_filter = "info"` (`core/observability/mod.rs:32`) suppresses. A validator built from the sample is a functioning DKG participant that attests nothing and says nothing. Supported, and this is the one an operator is most likely to hit.

No `H` claims.

### Resolving H-R6-15, which the reviewer left open

`../state/coverage-logs.md#r6` records the `#[serde(flatten)]` / `deny_unknown_fields` interaction as "**Unresolved, and deliberately not claimed**". I can close it further than R6 did, because the missing piece is not in `serde` but in this repository: `Config` carries `#[serde(deny_unknown_fields)]` (`config.rs:21`) _and_ `#[serde(default, flatten)] pub driver: driver::Config` (`:37-38`), and `driver::Config` is declared `#[derive(Clone, Debug, Default, Deserialize, PartialEq)] #[serde(default)]` — **without** `deny_unknown_fields` (`crates/core/src/driver.rs:30-37`), unlike every other config struct in `core`, all five of which do carry it (`tx/mod.rs:70`, `index/blocks.rs:48`, `index/mod.rs:21`, `index/events.rs:72`, `observability/mod.rs:17`). So an unknown top-level key is routed into the flattened struct's deserializer, which accepts and discards it. The structural facts are `E2`; that `serde` silently drops rather than rejects the combination is documented upstream but its source is not on disk, so that step stays `I` under A6. Practical consequence: a mistyped optional top-level table (`[observabilty]`, `[indx]`) is accepted in silence and the validator runs on defaults. This belongs in this finding rather than in a separate file; the reviewer's own remediation ("add a test asserting an unknown top-level key is rejected") settles it in one line and should be taken.

### Finding verdict

**Confirmed — 72%.** Mechanism `E2` on all three specifics plus both supporting gaps; the trigger is "copy the shipped sample and fill in the blanks", which needs no attacker and is verified against the sample file itself. Held below 85 because the harms are conditional on operator behaviour I cannot observe, and because item 2's consequence (a shared genesis group id across deployments) needs a second deployment to matter.

**Severity: Medium (unchanged).** Correct. A1 grants an honest operator, not an infallible one, and these are missing-validation defects with contained impact — the node fails silently rather than producing a wrong attestation. Not High: no attacker input is involved and nothing here loses funds or leaks key material. Not Low: item 3 makes the _shipped example_ produce a validator that never does its primary job, and item 1 is a consensus parameter with no cross-check, which is exactly the class of thing that is discovered only when an epoch fails to roll over.

**Remediation note.** Of the reviewer's options I would prioritise the cheapest two: make `oracles` either required or loudly warned about when empty at startup, and add the unknown-key test. The database-to-chain binding (a `meta(chain_id, consensus, coordinator)` row checked on open) is the highest-value structural change and also closes the "repoint at a different deployment" hazard that F-VAL-033 depends on operators avoiding.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **72%**; severity Medium unchanged. No PoC directory — not in my assigned set, and every test this finding needs belongs in `crates/validator/src/config.rs`'s existing test module rather than under `poc/`, which already has a `parses_sample_config` test to extend.

### What would be run, and what it would show

Three tests, all cheap, none needing a harness:

1. `toml::from_str::<Config>` on a document with an unknown top-level key. This settles the `#[serde(default, flatten)]` question on `driver: driver::Config` (`crates/validator/src/config.rs:36-37`), which is the one open point in the finding: `flatten` is known to weaken `deny_unknown_fields`, but `serde`'s source is not on disk (A6) so the reviewer could not confirm it. **This test answers it without reading `serde`**, which makes it the highest-value item here and I would put it first.
2. `ValidatorConfig::validate` rejects each dangerous combination once it exists.
3. `parses_sample_config` additionally asserts the parsed sample passes `validate`, so the shipped sample cannot drift into a configuration that does not work.

None of these would move the certainty, which is bounded by "the harms are conditional on operator behaviour I cannot observe" — C-VAL-B's reason, and it is right. Test 1 is the exception: it would convert one `I`-class supporting claim to `E1`, which is worth doing for its own sake even though it does not change the band.

### Remediation check

**Option 1 (a `ValidatorConfig::validate` called from `ValidatorService::new`) is sound and is the core of the fix.** It belongs exactly where the option puts it, beside the existing `InvalidValidators` check (`service/mod.rs:50-57`). Three refinements:

- **`oracles` empty should be a hard error, not a `warn!`.** The option offers both. A validator with no oracles never attests any transaction — its primary job — and it reports healthy while not doing it. That is the worst failure mode in the whole finding and it should not be reachable by omission. If a deliberately oracle-less deployment is a real mode, give it an explicit `oracles = []` opt-in rather than a default.
- **`key_gen_timeout * 3 >= blocks_per_epoch` is the right shape but the constant needs justifying in the code.** Write the derivation as a comment: three rounds (commitments, shares, confirmations) each bounded by `key_gen_timeout`, and the ceremony must finish inside one epoch. A bare magic `3` will be changed by someone who does not know why it is 3.
- Add `consensus` and `coordinator` not appearing in `oracles`, which is also **F-VAL-060** option 3's first half — implement it once, here.

**Option 3 (bind the database to its deployment) is the highest-value structural change in the file and I would raise it above option 2 in priority.** C-VAL-B says the same. It is one table, one check on open, and it closes three things at once: the "repoint at a different deployment" hazard here, the same gap on the sentinel side (CORE-H17), and the operational precondition **F-VAL-033** currently relies on operators avoiding. Note that the natural place to put it is `safenet_core::utils::connect_sqlite` or a new `core` helper, since both services need it — that makes it a core change, which the option does not say.

**Option 2 (anchor `blocks_per_epoch` onchain, or alert on divergence) is sound, and the cheap half is the one to take.** Reading it from `Consensus` needs a contract change; subscribing to `EpochStaged`/`EpochRolledOver` and `error!`-ing when the observed `rolloverBlock` disagrees with the local computation needs no contract change and converts silent exclusion into an alertable event. Take the second half now and treat the first as a protocol item.

**Option 4 (make `genesis_salt` required) is sound.** The current `#[serde(default)]` presents `B256::ZERO` as a neutral placeholder when it actually means "no deployment separation", and two deployments with the same participant set then share a genesis group id. Removing the default forces a decision. Cheap and correct.

**Option 5 (make the sample represent a working deployment) is sound and is the one that would have prevented the problem.** The sample is what operators copy; a sample that parses but produces a validator that never attests is a defect in the shipped artefact, not a documentation gap. Pair it with option 1's hard error on empty `oracles` so the two cannot disagree.

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against PR #902 ("[Phase 2] Adjust config"). **Effect: unchanged.** `crates/validator/src/config.rs` takes +7 lines on `origin/feat/batex_4` and all seven are inside the existing `deserializes_config` test (two assertions on the new `[transactions]` keys, plus the TOML lines that feed them). No validation is added anywhere: `participants`, `blocks_per_epoch`, `genesis_salt` and `oracles` still have no onchain source, no cross-validation and no startup consistency check beyond `ValidatorService::new`'s genesis-group test. `validator.sample.toml` on `feat/batex_4` still ships `genesis_salt = "0x0000…0000"` as an explicit uncommented setting and still leaves `oracles` commented out, so points 2 and 3 of the claim are reproducible verbatim against the branch tip. The stack adds two further unvalidated `[transactions]` keys (`executor`, `max_batch_gas`) — same house pattern, tracked under `F-CORE-066`. Severity and certainty unchanged. See `rust-audit/report/IN-FLIGHT.md`.
