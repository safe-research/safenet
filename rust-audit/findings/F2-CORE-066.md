# F2-CORE-066 The transactions table is bound to neither chain id, signer address nor schema version; a signer rotation or RPC change resumes with the old nonce bookkeeping and a schema change is a startup crash loop

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | safenet-core, tx/storage.rs, tx/mod.rs |
| Location | crates/core/src/tx/storage.rs:69-80 (related: tx/mod.rs:44-55, 110-126, 246; tx/storage.rs:145-149, 166, 309) |
| Severity | Low (reviewer) / Low (Critic) |
| Certainty | 70% (Critic C2-CORE-B; reviewer self-estimate 65%) |
| Assumptions involved | A1, A17 |
| Tags | config, crash-consistency |

## Claim

Rows carry a nonce and fee floor but nothing that identifies whose nonce space they belong to. `chain_id` is taken from the provider at signing time and the signer address from the configured key; neither is persisted or compared against what the table was built with. Two ordinary configuration changes, made without touching the database (so within A17), therefore go wrong silently:

- Rotating the signer key. In-flight rows keep nonces from the old account (say 50..55). Allocation is `MAX(new_account_nonce = 0, MAX(nonce) + 1 = 56)`, so the new key's first transactions are signed with nonce 56 and accepted as future-nonce transactions that can never mine; they are fee-bumped every two blocks (F2-CORE-060). The old rows are resubmitted signed by the new key and rejected. Nothing is logged above warn.
- Pointing an existing database at a different chain (or a different deployment on the same chain). The rows are replayed onto the new chain with the recorded nonces and floors; `chain_id` in the signature is the new chain's, so they are valid transactions there.

Separately, there is no schema version: the table is created with `CREATE TABLE IF NOT EXISTS` and `request` is free-form JSON deserialised into the current `AllocatedTransaction`. Any future change to `Transaction`'s fields turns an existing row into a `Serialization` error on the first `next_transaction`/`stale_submissions`, which `is_intermittent` classifies as fatal, so an upgraded service exits on every start until the row is removed by hand.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The schema has no chain, signer or version column and is created idempotently. | E2 | crates/core/src/tx/storage.rs:69-80 | `sqlx::query(` / `"CREATE TABLE IF NOT EXISTS transactions (` / `id           INTEGER PRIMARY KEY,` / `request      TEXT    NOT NULL,` / `expires_at   INTEGER DEFAULT NULL,` / `nonce        INTEGER DEFAULT NULL,` / `submitted_at INTEGER DEFAULT NULL,` / `executed_at  INTEGER DEFAULT NULL` / `)",` |
| 2 | Chain id and signer are taken live, never compared with stored state. | E2 | crates/core/src/tx/mod.rs:246-248, 310 | `let chain_id = self.provider.chain_id();` / `let fees = self.fees().await?;` / `let transaction = transaction.build(chain_id, fees);` ... `.get_transaction_count(self.signer.address())` |
| 3 | Allocation continues from the stored maximum regardless of the account. | E2 | crates/core/src/tx/storage.rs:145-149 | `"UPDATE transactions` / `SET nonce = MAX(?, COALESCE(` / `(SELECT MAX(nonce) + 1 FROM transactions),` / `0` / `))` |
| 4 | Row deserialisation errors are storage errors ... | E2 | crates/core/src/tx/storage.rs:18-20, 166 | `/// A transaction request could not be serialized or deserialized.` / `#[error("failed to serialize or deserialize a transaction request")]` / `Serialization(#[from] serde_json::Error),` ... `let transaction = serde_json::from_str::<AllocatedTransaction>(&request)?;` |
| 5 | ... and storage errors are fatal by design. | E2 | crates/core/src/tx/mod.rs:47-54 | `fn is_intermittent(&self) -> bool {` / `// Note that we only consider RPC errors as transient - everything else` / `// including SQLite errors (which only happen if you are in a pretty` / `// borked FS situation or there is a bug in the SQL logic) and signing` / `// errors ... are` / `// considered more serious.` / `matches!(self, Self::Rpc(_))` |
| 6 | No migration framework exists in the crate. | E2 | crates/core/src/tx/storage.rs:61-83, crates/core/Cargo.toml:7-31 (no `sqlx::migrate!`, no `migrations/` directory: `find crates/core -name '*.sql'` returned nothing this session) | `pub async fn new(pool: SqlitePool) -> Result<Self, Error> {` ... `.execute(&pool)` / `.await?;` / `Ok(Self { pool })` |

## Trigger

Rotate `signer` in the config while any row is in flight (or executed but not yet pruned), restart. Observe nonce 56-style allocations for the new key. Or: upgrade to a build that adds a field without a serde default to `Transaction`; the service exits at the first block with `failed to serialize or deserialize a transaction request`.

## Considered and rejected

- _A17 puts database mishandling out of scope._ A17 covers out-of-band database access; changing `signer` or `rpc` in the config is in-band and the database is untouched.
- _Operators are told never to reuse keys._ The handbook warning (about external use of the same key) is the opposite direction; rotating to a fresh key is the recommended reaction to a suspected leak and is precisely when this bites.
- _Serde defaults will always be added._ Perhaps; there is no test or version guard enforcing it.

## Remediation options

1. Add a one-row `meta` table (`chain_id`, `signer`, `schema_version`) written on first creation and checked in `TransactionStorage::new`; refuse to start on mismatch with an explicit error naming the fix (new database path, or a documented reset).
2. Store `chain_id` and `signer` per row and scope every query to the current pair; old rows become inert instead of poisoning allocation.
3. Adopt `sqlx::migrate!` for both tables so schema changes are explicit.

Tests to add: construct storage, insert a row, reopen with a different signer and assert a startup error.

## Trail

- Reviewer R3: drafted at commit 3ec8bc5, self-estimate 65%
- Critic C2-CORE-B: Confirmed, 70%, severity Low (reviewer Low).

## Critic (C2-CORE-B)

Method: read title and Location only, traced the schema, allocation and signing paths myself, then compared.

| # | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | storage.rs:69-80. |
| 2 | Supported | mod.rs:246-248 and 310. |
| 3 | Supported | storage.rs:145-149. |
| 4 | Supported | storage.rs:18-20, 166. |
| 5 | Supported | mod.rs:47-54. |
| 6 | Supported | `find crates/core -name '*.sql'` → 0 files and no `migrate!` in `crates/core/src` (re-run this session). |

Own trace of the rotation case: stored rows keep their nonces (`types.rs:64-77` signs with `self.nonce`), the new key's reading is 0, and `next_transaction` allocates `MAX(0, MAX(nonce) + 1)` (storage.rs:146-149) — above the new account's real count — so nothing mines and the queue stalls until the database is replaced. The trigger is a config change, which A17 leaves in scope; the remedy being out-of-band does not affect scope. The schema-versioning half has no concrete trigger at this commit and carries Informational weight inside the finding.

Finding verdict: **Confirmed** (mechanism E2; trigger is a concrete operator action). Certainty **70%**. Severity **Low / Low**.

Related, not filed: the snapshot store has the same unversioned-JSON property (`snapshots.state`, state/storage.rs:70-77; R2's observation O10) — same remediation 3.

## Reconciliation (run 2)

**Final: CONFIRMS `F-CORE-065` (canonical, `known`) and the version half of `F-CORE-037` — combined Low, 70 (E2).** A17 narrows the run-1 trigger set (reused/copied/restored databases out of scope; signer rotation and `rpc` repoint in scope), exactly as this file's header already reads (`state/run2/reconciliation/core.md` §3).
