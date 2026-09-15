# F-CORE-037 Snapshots are an unversioned JSON dump of the service state with no migration path and no recovery from a decode failure: an upgrade that changes a state type bricks start-up, a downgrade silently discards fields

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `state/storage.rs` |
| Location | `crates/core/src/state/storage.rs:47-79` and `:103-116` (related: `crates/core/src/state/mod.rs:119-151`; `crates/validator/src/state/mod.rs:36-54`; `crates/sentinel/src/state.rs:24-94`) |
| Severity | Low / Low |
| Certainty | 62% |
| Assumptions involved | A1 |
| Tags | config, crash-consistency |

## Claim

The `snapshots` table is `(block_number INTEGER PRIMARY KEY, state TEXT NOT NULL)` where `state` is `serde_json::to_string` of the service's `State` type. There is no schema version column, no `PRAGMA user_version`, no `sqlx::migrate!` anywhere in the workspace (every table in the repository is a bare `CREATE TABLE IF NOT EXISTS`), and no format tag inside the JSON. The service `State` types are plain derives with no `#[serde(default)]` and no `#[serde(other)]` fallbacks (`crates/validator/src/state/mod.rs:37-54`, `crates/sentinel/src/state.rs:24-94`).

Two consequences, both on the ordinary upgrade path:

- **Forward (upgrade): loud but unrecoverable.** Adding a field to any state type — or renaming one, or changing a type — makes every persisted snapshot fail to deserialize. That surfaces as `storage::Error::Serialization` from `SnapshotStore::current`, which propagates through `StateMachine::with_init` and `Driver::new` and aborts start-up. There is no fallback path: the state machine can only start from the persisted snapshot or from an _empty_ store, and no code offers the second when the first fails to decode. The operator's only recovery is to delete rows by hand and re-index from `start_block`, and nothing in the code or the handbooks tells them so.
- **Backward (rollback): silent.** `serde_json` ignores unknown fields by default and no state type sets `deny_unknown_fields` (unlike the _config_ types, which do — `observability/mod.rs:17`, `tx/mod.rs:70`). Rolling a binary back therefore loads the newer snapshot, silently drops whatever the new version added, and continues from a state that is quietly wrong, committing that truncated state back over the row on the next block (`commit` is an upsert, `storage.rs:107-114`).

Neither failure mode is exotic: the epic on file (`epics/2026_07_14_validator_state_machine_flow_test_harness.md`, assumption A12) is about extending the validator state machine, and every added field is a compatibility break under this design.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The table has two columns and no version or format marker. | E2 | `crates/core/src/state/storage.rs:49-57` | <pre>pub async fn new(pool: SqlitePool) -> Result<Self, Error> {<br> sqlx::query(<br> "CREATE TABLE IF NOT EXISTS snapshots (<br> block_number INTEGER PRIMARY KEY,<br> state TEXT NOT NULL<br> )",<br> )<br> .execute(&pool)<br> .await?;</pre> |
| 2 | The state column is untagged `serde_json`. | E2 | `crates/core/src/state/storage.rs:105-114` | <pre>pub async fn commit(&self, block_number: u64, state: &S) -> Result<, Error> {<br> let state = serde_json::to_string(state)?;<br> sqlx::query(<br> "INSERT INTO snapshots (block_number, state) VALUES (?, ?)<br> ON CONFLICT (block_number) DO UPDATE SET state = excluded.state",<br> )</pre> |
| 3 | A decode failure on load is an error with no fallback. | E2 | `crates/core/src/state/storage.rs:69-79` | <pre>pub async fn current(&self) -> Result<Option<(u64, S)>, Error> {<br> sqlx::query_as::<_, (i64, String)>(<br> "SELECT block_number, state FROM snapshots ORDER BY block_number DESC LIMIT 1",<br> )<br> .fetch_optional(&self.pool)<br> .await?<br> .map(&#124;(block_number, state)&#124; {<br> Ok((u64::try_from(block_number)?, serde_json::from_str(&state)?))<br> })<br> .transpose<br>}</pre> |
| 4 | The only "start fresh" path is taken when the store is _empty_, never when it fails to decode. | E2 | `crates/core/src/state/mod.rs:134-143` | <pre>let snapshots = SnapshotStore::new(pool).await?;<br>let (state, status) = snapshots<br> .current<br> .await?<br> .map(&#124;(latest, state)&#124; -> Result<_, Error> {<br> let pending = latest.checked_add(1).ok_or(Error::EndOfChain)?;<br> Ok((state, Status::BlockPending { pending }))<br> })<br> .transpose?<br> .unwrap_or_else(&#124;&#124; (init, Status::Initialized));</pre> |
| 5 | Service state types are plain derives: no field defaults, no unknown-field policy, no version. | E2 | `crates/validator/src/state/mod.rs:36-54` | <pre>/// The complete snapshotted validator state.<br>#[derive(Clone, Debug, Default, Deserialize, Serialize)]<br>pub struct State {<br> /// The epoch-rollover / DKG state machine.<br> rollover: RolloverState,<br> /// The epoch whose group is currently active in consensus.<br> active_epoch: EpochId,</pre> |
| 6 | Same for the sentinel. | E2 | `crates/sentinel/src/state.rs:92-94` | <pre>/// Snapshot state: every in-flight request, keyed by request ID.<br>#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]<br>pub struct State(pub HashMap<B256, SentinelRequestState>);</pre> |
| 7 | Config structs _do_ use `deny_unknown_fields`, so the absence on the persisted types is a gap rather than a house style. | E2 | `crates/core/src/observability/mod.rs:16-18` | <pre>#[derive(Clone, Debug, Deserialize)]<br>#[serde(default, deny_unknown_fields)]<br>pub struct Config {</pre> |
| 8 | No migration machinery exists anywhere in the workspace. | E2 | repository-wide grep run this session: `grep -rn "migrate\|schema_version\|user_version\|PRAGMA" crates/ --include=*.rs` | Only `sentinel-engine` Safe-contract selectors (`migrateSingleton` etc.) match; no `sqlx::migrate!`, no `user_version`, no `PRAGMA`. |

## Trigger

1. A release adds one field to `validator::state::State` (say a new `BTreeMap` for a protocol feature) without `#[serde(default)]` — the natural way to write it, and what every existing field does.
2. The upgraded binary starts against the existing database. `SnapshotStore::current` reads the tip row, `serde_json::from_str::<State>` fails with `missing field`, `StateMachine::with_init` returns `Err(Storage(Serialization(..)))`, `Driver::new` returns it and `main` exits with the error.
3. The validator does not start. Restarting does not help; the error names a serde field, not a remedy. The operator has to discover that deleting `snapshots` (but _not_ the validator's `keygen_secrets`/`nonces` tables, which live in the same file and must survive — `crates/validator/src/secrets/store.rs:67-88`) is the fix, then re-index from `start_block`.
4. Rolling back to the previous binary after step 2 succeeds, but the rows written by any node that did run the new version silently lose the new field, and the next `commit` upserts the truncated state over the row for good.

## Considered and rejected

- **"Wiping the snapshot table is a cheap recovery."** It is cheap only if the state is derivable from chain history alone. It is not always: the validator's snapshot holds the canonical nonce assignments and epoch bookkeeping that pair with the _non_-rolled-back secret store, so a wipe and re-index changes the relationship between the two stores. This finding does not claim that is unsafe — the reconciliation path (`ReconcileGroupSecrets`) exists — only that no documented, tested recovery procedure exists at all.
- **"Every service has this problem."** Most services with a persisted, derived state have either a version column, a `#[serde(default)]` discipline, or a documented "delete and re-sync" runbook. Here there are none of the three, and the failure mode is a service that cannot start.
- **"It is caught in staging."** The forward case is; the backward case is not, because it produces no error at all.
- **Not a duplicate of F-CORE-001 / CORE-H17** (no chain-id or fork binding of the database): that is about the snapshot belonging to the wrong _chain_, this is about it belonging to the wrong _code version_. A single `metadata` table would be a natural place to fix both.

## Remediation options

1. Add a `metadata` table (or `PRAGMA user_version`) holding a snapshot-format version, written by `SnapshotStore::new` and checked on open; refuse to start on a mismatch with an explicit message naming the supported versions and the remedy. Cheap, and it is the same place CORE-H17's chain-id binding would go.
2. Make compatibility the default in the type system: require `S: Default` on the load path and give every service-state field `#[serde(default)]`, plus `#[serde(deny_unknown_fields)]` on the state types so a _downgrade_ fails loudly instead of silently truncating. Tradeoff: `deny_unknown_fields` turns the silent rollback into a hard stop, which is the correct trade for consensus state but must be a deliberate choice.
3. Offer a recovery path rather than only a failure: on a decode error, log loudly and treat the store as empty (re-indexing from `start_block`) only when an explicit `--reset-state-on-incompatible-snapshot` flag is given. Never do it silently — the state is consensus-relevant.
4. Whatever is chosen, document it in the two handbooks next to the existing "ensure this survives restarts" guidance (`docs/validator-handbook.md:75-79`).

Tests to add: a `state/storage.rs` test that a snapshot written by a struct with an extra field fails (or succeeds, per the chosen policy) when read back into the older struct; a version-mismatch test for the metadata table. No code is committed.

## Trail

- Reviewer R2: drafted from core checklist item 12 and the brief's "serialization round-trip stability of snapshots across versions" question, self-estimate 75%. Mechanism is `E2` throughout; the severity is Low because it needs an upgrade, not an attacker.

## Critic (C-CORE-B)

Read `state/storage.rs:47-79`, `:103-116` and `state/mod.rs:129-151` first. The table is `(block_number INTEGER PRIMARY KEY, state TEXT NOT NULL)`; `commit` writes `serde_json::to_string(state)` with no tag; `current` propagates a `serde_json::Error` as `storage::Error::Serialization`; and `with_init` reaches `init` only via `unwrap_or_else` on `Option`, i.e. only when the store is **empty**, never when a row fails to decode. So a decode failure is a hard start-up abort with no fallback. Confirmed independently.

### Per-claim verdicts

Rows 1-7 **Supported**, verbatim at the cited ranges. I re-ran row 8's grep myself: `grep -rn "migrate|user_version|PRAGMA" crates/ --include=*.rs` returns only the Safe-contract `migrateSingleton`/`migrateL2Singleton` selectors in `sentinel-engine`; there is no `sqlx::migrate!`, no `PRAGMA user_version`, no version column anywhere. Row 5 is understated rather than wrong: `crates/validator/src/state/mod.rs:36-54` has no `#[serde(default)]` on any of its five fields and no container-level attribute at all, so the forward break is certain for any added field.

The backward half is also correct: `serde_json` ignores unknown fields by default, none of the state types sets `deny_unknown_fields` (unlike `observability::Config` at `mod.rs:17` and `tx::Config` at `tx/mod.rs:70`), and `commit` is an upsert (`storage.rs:107-114`), so a downgraded binary writes the truncated state back over the row.

### Finding verdict

**Plausible — 62%.** The mechanism is `E2` and airtight; what keeps it out of the Confirmed band is that the trigger is a _future_ release rather than anything reachable in this tree — no state type has changed shape yet, so nothing today fails to decode. That is the same shape as F-CORE-038 (a hazard with no current trigger) and belongs in the 40-69 band by the rubric.

**Severity: Low (unchanged).** Correct. The forward failure is loud and repairable by an operator who understands it; the backward failure is silent but requires a deliberate downgrade after a schema change. Neither is reachable from untrusted input, so this is a robustness and operability gap, not a security one. The reviewer's point that no runbook exists for either is the part worth acting on.

One caution for the remediation: option 2's `deny_unknown_fields` on state types would convert every rollback into a hard stop, which interacts with F-CORE-030 (the stop would exit with status 0). Options 1 and 2 should land together with F-CORE-030's fix, not before it.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, with option 3's flag. Option 2's second half needs care.**

Option 1 (a `metadata` table or `PRAGMA user_version` holding a snapshot-format version, checked on open, refusing to start on a mismatch with an explicit message) is sound and is the right first change. It is also the **same table** F-CORE-001 option 4 and F-CORE-065 option 1 ask for — three findings, one single-row metadata table. Implement it once; three separate tables would be a worse outcome than none.

Option 2's first half (`S: Default` on the load path, `#[serde(default)]` on state fields) is sound and is what makes a forward upgrade survivable. Its second half — `#[serde(deny_unknown_fields)]` on state types so a _downgrade_ fails loudly — is the correct trade for consensus state, but note it interacts with a known serde subtlety: `deny_unknown_fields` and `#[serde(flatten)]` do not compose, which is question 3 in `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md` and is the subject of F-XC-003 / F-VAL-063. If any state type uses `flatten`, the attribute may be silently inert. **Check that before relying on it.**

Option 3 (recover by treating the store as empty only behind an explicit `--reset-state-on-incompatible-snapshot` flag, never silently) is sound and the "never silently" qualifier is the whole value of the option — re-indexing from `start_block` discards consensus state and must be an operator decision.

Option 4 (document next to the existing "ensure this survives restarts" guidance) is necessary; the handbooks currently tell operators to preserve a file whose upgrade behaviour is undefined.

## In-flight impact (AS-SEN)

**Pertains to unmerged branches, not to `main`.** Assessed against `origin/fix/sentinel_deadlines` (PR #914) and `origin/feat/optimistic_block_transition` (PR #915, tip `b2aad06`), built from a `git archive` extraction. Both apply cleanly onto `main` — `main`'s crates are byte-identical to their base `199629e`, so a verdict on the tip is also the verdict for the tip merged into `main`. **Effect: unchanged; no migration needed.** #915's new `Status` exists only in memory (`crates/core/src/state/mod.rs:121-137`), and snapshot content semantics are unchanged; the format is still unversioned.

## Scope note (run 2)

`crates/sentinel-engine` was placed out of scope after this finding was written (PROMPT.md Section 11); the engine-specific parts above are retained as run-1 history and are not part of the current audit's claims. The non-engine parts stand.
