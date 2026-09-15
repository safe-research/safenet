# F-CORE-065 No chain-id or deployment binding on the `transactions` table, and `Provider::chain_id` is cached at connect so an endpoint chain change is undetectable

| Field | Value |
| --- | --- |
| Status | Critiqued |
| Crate and module | core, `tx/storage.rs`, `tx/mod.rs` (evidence from `provider/mod.rs`) |
| Location | `crates/core/src/tx/storage.rs:58-83` (related: `crates/core/src/tx/mod.rs:106-126, 241-256`, `crates/core/src/provider/mod.rs:127-165`) |
| Severity | Low / Low |
| Certainty | 55% |
| Assumptions involved | A1, A4 |
| Tags | config, crash-consistency, known |

## Claim

The `transactions` table records nonces, submission blocks and fees for one specific chain and one specific signing account, but stores nothing that identifies either. There is no chain id, no contract address, no signer address, no schema version, and the table is created with `CREATE TABLE IF NOT EXISTS` rather than a migration. Pointing an existing database at a different chain, a different deployment, or configuring a different signer key against the same file is accepted in silence, and the queue immediately resumes with nonces, fee floors and submission blocks that belong to a different account or a different chain.

The one value that could catch this — the chain id — is read once when the provider connects and then served from memory forever. `Provider` overrides `get_chain_id` to return the cached value without issuing a request, so no code path anywhere in the process can observe that the endpoint's chain changed. The chain id is nonetheless what every signature commits to (`TxEip1559::chain_id`), so it is load-bearing for replay protection while being unverified for the life of the process.

This is `analysis-core.md` H17's neighbourhood and is filed at reduced priority per A12, but the queue-specific consequences below are not covered there.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The schema carries no chain, deployment, signer or version column, and is created idempotently rather than migrated. | E2 | `crates/core/src/tx/storage.rs:61-82` | <pre>pub async fn new(pool: SqlitePool) -> Result&lt;Self, Error&gt; {<br> // Note that we store the `nonce` in a separate column from the<br> // transaction request JSON data. This allows us to work more naturally<br> // with the `nonce` column (for things like `MAX` to determine the next<br> // nonce), which would be more verbose if it were part of the request<br> // data directly (as we would need JSON extractors to use the column<br> // and would have to potentially deal with hexadecimal encoding, to<br> // match other numerical values are serialized).<br> sqlx::query(<br> "CREATE TABLE IF NOT EXISTS transactions (<br> id INTEGER PRIMARY KEY,<br> request TEXT NOT NULL,<br> expires_at INTEGER DEFAULT NULL,<br> nonce INTEGER DEFAULT NULL,<br> submitted_at INTEGER DEFAULT NULL,<br> executed_at INTEGER DEFAULT NULL<br> )",<br> )<br> .execute(&pool)<br> .await?;</pre> |
| 2 | The queue is constructed with no chain id and stores none; the doc comment claims a `chain_id` the signature does not have. | E2 | `crates/core/src/tx/mod.rs:106-116` | <pre>impl TransactionQueue {<br> /// Creates a transaction queue that signs `chain_id` transactions with<br> /// `signer`, reads chain state and broadcasts through `provider`, and<br> /// persists its state in `pool`.<br> pub async fn new(<br> provider: Provider,<br> signer: Signer,<br> pool: SqlitePool,<br> config: Config,<br> ) -> Result&lt;Self, Error&gt; {<br> let storage = TransactionStorage::new(pool).await?;</pre> |
| 3 | The chain id is taken from the provider at submission time and goes straight into the signed payload. | E2 | `crates/core/src/tx/mod.rs:245-248` | <pre>) -> Result&lt;, Error&gt; {<br> let chain_id = self.provider.chain_id;<br> let fees = self.fees.await?;<br> let transaction = transaction.build(chain_id, fees);</pre> |
| 4 | The provider reads the chain id once, at connect. | E2 | `crates/core/src/provider/mod.rs:129-137` | <pre>pub async fn connect(url: &Url) -> Result&lt;Self, TransportError&gt; {<br> let client = ClientBuilder::default<br> .layer(ObservabilityLayer)<br> .connect(url.as_str)<br> .await?;<br> let root = RootProvider::new(client);<br> let chain_id = root.get_chain_id.await?;<br> Ok(Self { root, chain_id })<br>}</pre> |
| 5 | `get_chain_id` is overridden to return the cached value without an RPC call, so nothing downstream can re-check it. | E2 | `crates/core/src/provider/mod.rs:158-165` | <pre>impl AlloyProvider&lt;AnyNetwork&gt; for Provider {<br> fn root(&self) -> &RootProvider&lt;AnyNetwork&gt; {<br> &self.root<br> }<br><br> fn get_chain_id(&self) -> ProviderCall&lt;NoParams, U64, u64&gt; {<br> ProviderCall::Ready(Some(Ok(self.chain_id)))<br> }<br>}</pre> |
| 6 | Fee floors are persisted per row, so stale fees from a previous deployment are reused on the next submission. | E2 | `crates/core/src/tx/storage.rs:176-185` | <pre>let updated = sqlx::query(<br> "UPDATE transactions<br> SET submitted_at = ?,<br> request = json_set(<br> request,<br> '$.maxFeePerGas', ?,<br>             '$.maxPriorityFeePerGas', ?<br> )<br> WHERE nonce = ?",<br>)</pre> |
| 7 | The signer address is used to fetch the nonce but is never compared against anything persisted. | E2 | `crates/core/src/tx/mod.rs:308-312` | <pre>let nonce = self<br> .provider<br> .get_transaction_count(self.signer.address)<br> .block_id(block_id)<br> .await?;</pre> |

## Trigger

Three operator-reachable sequences, all silent:

1. **Reused database across chains.** A validator is tested against a devnet or testnet and the same SQLite file (or a copy of it, or a restored backup) is then used against mainnet, or the `rpc` URL in the config is repointed. The `transactions` table still holds rows with nonces, `submitted_at` blocks and fee floors from the other chain. Claim 1 shows nothing rejects them; claim 6 shows the old fees are reapplied; and since block numbers differ between chains, `prune` and `stale_submissions` classify those rows arbitrarily — a row whose `submitted_at` is a high testnet block will not be considered stale on a lower-numbered chain, so it holds its nonce and blocks the queue (`F-CORE-062`) until the head passes it.
2. **Changed signer key, same database.** The config's `signer` is rotated but the database is kept. The nonces in the table belong to the old account; the chain nonce fetched for the new account (claim 7) is unrelated. `MAX(nonce)+1` is floored at the old account's high-water mark, so the new account's transactions are allocated nonces far above its true count — a permanent gap, i.e. `F-CORE-062`'s wedge reached without any RPC anomaly at all.
3. **Endpoint switching chains under the process.** Under A4 the RPC is trusted but may be inconsistent; a misconfigured load balancer or a provider migration that moves a URL between networks is an availability-class event, not a malicious one. Claims 4 and 5 mean the process keeps signing with the chain id it learned at startup for as long as it runs. Those signatures are valid on the _original_ chain, so nothing is forged, but every transaction is rejected by the new endpoint while the queue interprets the rejections through `is_transaction_underpriced` and, per `F-CORE-060`, may ratchet fees against a chain it is not on.

Sequences 1 and 2 need only ordinary operations under A1 (trusted but fallible operator) and are the reason this is worth more than a documentation note: sequence 2 in particular produces a permanently wedged queue from a routine key rotation.

I have executed none of these (A9 FALSE); claims 1–7 are `E2` from the cited code, and the sequences are `I`.

## Considered and rejected

- **"A1 says the operator is trusted, so this is out of scope."** A1 says config, keys and filesystem are provisioned by an honest operator; it does not say the operator is infallible, and every finding whose trigger is a plausible operational mistake still counts under the "configuration weaknesses with limited impact" severity band. The reason it is filed Low rather than higher is the trusted-operator assumption, not an absence of impact.
- **"H17 already covers it, so it is `known`."** `analysis-core.md` H17 records the absence of chain-id binding as Informational and stops there. It does not identify the `get_chain_id` override (claims 4–5), which is what makes the condition undetectable rather than merely unrecorded, nor sequence 2 (signer rotation), which reaches `F-CORE-062`'s wedge with no RPC anomaly. It is not in `codebase-map.md` §4, so the A12 `known` tag applies only loosely; I have tagged it `known` anyway and filed at reduced priority, per A12's conservative reading.
- **"The signature would be replayable on the other chain."** It would not. `chain_id` is part of the signed `TxEip1559` payload (`crates/core/src/tx/types.rs:64-72`), so a transaction signed for chain A is invalid on chain B. Cross-chain _replay_ is correctly prevented; what is not prevented is the queue's bookkeeping being wrong, which is a liveness and cost problem, not a safety one. This is why the severity is Low.
- **"`state/storage.rs` has the same gap, so this is one cross-module finding."** The `snapshots` table has the same absence, but it is another reviewer's file and the queue-specific consequences (nonce high-water mark, persisted fee floors, `submitted_at` block numbers) are distinct. I cite `provider/mod.rs` as evidence only and file no finding on it.
- **"`CREATE TABLE IF NOT EXISTS` is fine because the schema never changes."** It has no version column, so the first schema change will have no way to detect or migrate an old file. This is a latent rather than an active defect and is noted here rather than filed separately.

## Remediation options

1. **Bind the database to its deployment.** Add a single-row `metadata` table holding chain id, signer address, and the watched contract addresses, written on first creation and verified on every open. A mismatch should be a startup error naming the two values, not a warning. This is the smallest change that closes all three sequences.
2. **Verify the chain id periodically, or at least re-verify on reconnect.** Keep the cache for the hot path, but re-issue `eth_chainId` on a timer or whenever the transport reconnects, and treat a change as fatal. Tradeoff: one extra request per interval; the current override (claim 5) means the check has to be added explicitly since callers cannot force it.
3. **Add a schema version column** and refuse to open a database whose version the binary does not know, so the absence of `sqlx::migrate!` does not become a silent-corruption path later.
4. **Fix the stale doc comment** at `crates/core/src/tx/mod.rs:107-109`, which promises a `chain_id` binding the constructor does not take. As written it is the sentence most likely to convince a reader that the binding exists.
5. **Document the operational rule** — one database per (chain, deployment, signer) — in the handbooks, alongside the existing "never reuse the key" guidance, and say what to do with the file after a key rotation.

Tests to add: a storage test that opens a database written with one signer/chain identity under another and asserts a startup error; a test that `Provider` surfaces a chain-id change (once re-verification exists).

## Trail

- Reviewer R3: drafted, self-estimate 65%. Claims 1–7 are `E2` and directly cited. Severity is deliberately Low under A1; the reason I filed it rather than leaving it as an observation is sequence 2, where a routine key rotation reaches `F-CORE-062`'s permanent wedge with no RPC anomaly required. Tagged `known` per A12 as an extension of `analysis-core.md` H17.

## Critic (C-CORE-B)

Read the schema and the provider first. `CREATE TABLE IF NOT EXISTS transactions (id, request, expires_at, nonce, submitted_at, executed_at)` (`tx/storage.rs:69-80`) carries no chain id, no signer address, no deployment identifier and no version, and there is no migration machinery anywhere in the workspace (I re-ran the grep for `sqlx::migrate!` / `PRAGMA user_version` — nothing). `Provider` caches `chain_id` at `connect` (`provider/mod.rs:129-137`) and then **overrides** `get_chain_id` to return the cached value with no request at all (`:163-165`), so no code path in the process can observe an endpoint changing chains. Both halves confirmed independently.

### Per-claim verdicts

All basis rows **Supported** against the cited ranges. The `get_chain_id` override is the strongest of them and is exactly as described: `ProviderCall::Ready(Some(Ok(self.chain_id)))`.

### Assessment

Sequence 2 (rotate the signer key, keep the database) is the sharpest of the three and needs no RPC anomaly whatsoever: the old account's nonces remain in the table, `MAX(nonce)+1` floors every new allocation above them, and the new account's true count is unrelated — F-CORE-062's permanent wedge reached from a routine operational step. That is a genuine `E2` mechanism with an `I` trigger (an operator action).

Sequence 3 is weaker than presented. If the endpoint moves to a different chain, transactions signed with the stale chain id are rejected by the new node with a chain-id error, which does not match `is_transaction_underpriced` (`tx/mod.rs:362-368`) and therefore takes the generic branch — so the fee does **not** ratchet, contrary to the "may ratchet fees against a chain it is not on" wording. It stalls instead. Minor overreach; it does not affect the verdict.

### Finding verdict

**Plausible — 55%.** Mechanism `E2` and complete; every trigger is an operator sequence, class `I`, as the reviewer states.

**Severity: Low (unchanged), `known` tag retained.** Correct. A1 makes the operator trusted, all three sequences are operator-initiated, and nothing here is reachable from chain data or from a provider acting within A4. It is a durable-state-binding gap of the same family as F-CORE-037's missing snapshot version — and the reviewer's cross-reference is right that a single `metadata` table would fix both, which is the practical recommendation for the report.

## QA (QA-CORE-SEN)

**Outcome: Not attempted (no toolchain).** No Rust toolchain exists on this host (`state/baseline.md` §1), so nothing was executed and no certainty moves. No PoC was written for this finding; the seven PoCs this run produced are in `rust-audit/poc/` for F-CORE-001, -002, -060, -067 and F-SEN-001, -002, -015. This section is a remediation-soundness check only.

### Remediation check

**Sound: option 1, and it is the third finding asking for the same table.**

Option 1 (a single-row `metadata` table holding chain id, signer address and the watched contract addresses, written on creation and verified on every open, with a startup **error** naming both values) is sound and is the smallest change that closes all three sequences. It is the **same table** as **F-CORE-001 option 4** and **F-CORE-037 option 1**. Three findings, one table — the report should consolidate them into a single recommendation, because three separately-implemented single-row metadata tables would be a worse outcome than none.

Option 2 (re-verify the chain id on reconnect, or on a timer) is sound and its stated obstacle is real: `Provider` overrides `get_chain_id` to return the cached value unconditionally (`crates/core/src/provider/mod.rs:163-165`), so callers _cannot_ force a fresh read. Any implementation has to add an explicit uncached path, which the option should say outright.

Option 3 (a schema version column) is the same change as F-CORE-037 option 1 and should be folded into it.

Option 4 (fix the stale doc comment at `tx/mod.rs:107-109`, which promises a `chain_id` binding the constructor does not take) is unconditionally correct and is the sentence most likely to convince a reader the binding already exists. One line; take it now.

Option 5 (document the operational rule — one database per chain/deployment/signer — and what to do with the file after a key rotation) fills a gap that currently has no answer anywhere in the handbooks.

## In-flight impact (FWD)

**Pertains to unmerged branches, not to `main`.** Assessed against the "Batched Execution" stack (`origin/feat/batex_0` … `origin/feat/batex_4`, PRs #899–#904). **Effect: worsen.** The stack neither binds nor further unbinds the table in the ways this finding names — there is still no chain id, no signer address, no deployment identifier and no schema version on `transactions`, and the DDL gains only a `transactions_nonce_idx` index — but it makes the un-verified chain id carry materially more weight. Phase 3 signs the EIP-7702 authorization as `Authorization { chain_id: U256::from(tx.chain_id), address: delegate, nonce: tx.nonce + 1 }` (`tx/signer.rs`), where `tx.chain_id` comes from the same `Provider::chain_id()` connect-time cache this finding flags as unverifiable for the life of the process. Using the real chain id rather than the wildcard `0` is the right call and closes cross-chain _replay_ — but it means a stale or wrong cached id no longer merely misdirects one transaction: it produces an authorization the chain rejects, leaving the transaction mined and the delegation unapplied, which is precisely the permanent nonce gap of the new `F-CORE-069`. Separately, the stored `request` JSON now gains an `authorization` field holding a bare executor **address**, so a database pointed at a different deployment now carries a code-delegation target across with it, at an address that may host entirely different code on the other chain. Severity and certainty unchanged; the "load-bearing while unverified" argument is strengthened. See `rust-audit/report/IN-FLIGHT.md`.
