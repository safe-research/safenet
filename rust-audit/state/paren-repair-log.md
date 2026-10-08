# Parenthesis repair log

A date-removal pass stripped empty `()` from function references in the generated audit files (findings, report, state, poc `.md`). The runnable PoC `.rs` sources and the git-tracked kit files were unaffected.

- **Restored, tier 1 (75 lines)**: the repaired line is byte-identical to a line in `crates/` or an intact PoC `.rs` file.
- **Restored, tier 2 (21 lines, one corrected by hand)**: names that are only ever functions in this codebase (`new`, `is_empty`, `unwrap`, `len`, `dummy`, `begin`, `default`, `expect`, `read_q`) followed by a Rust terminator.
- **Left untouched (21 lines)**: ambiguous between a method call and a field access, or calling a std/tokio/sqlx method not in this codebase. Listed below for review. Prose outside code fences was not repaired (cosmetic only).

One tier-2 edit was wrong and was corrected manually: `poc/V-XC-phase5/ANSWERED-QUESTIONS.md:48` — the rule mis-read the range `[..len]` as a `.len` call; the true damage on that line was `spare_capacity_mut` losing its `()`, now restored as `tokens.spare_capacity_mut()[..len]`.

## Unresolved lines

| File | Line | Text |
| --- | --- | --- |
| `findings/F-VAL-038.md` | 86 | `let signing_share = key_share.as_key_package.signing_share;` |
| `findings/F-VAL-060.md` | 267 | `// using the F-VAL-004 harness, with `log.address` set to an oracle address` |
| `findings/F-VAL-036.md` | 95 | `.find(\|epoch\| epoch.group.id == event.gid)` |
| `findings/F-VAL-036.md` | 211 | `let before = nonces.available;` |
| `findings/F-VAL-036.md` | 214 | `assert_eq!(nonces.available, before);         // FAILS TODAY: 192 becomes 1023` |
| `findings/F-VAL-035.md` | 137 | `query.build.execute(&self.pool).await?;` |
| `findings/F-VAL-037.md` | 108 | `require(MerkleProof.verifyCalldata(poap, self.root, leaf), NotParticipating);` |
| `findings/F-VAL-031.md` | 103 | `let permit = self.pending.clone.try_acquire_owned.ok;` |
| `findings/F-VAL-031.md` | 168 | `let signing_share = key_share.as_key_package.signing_share;` |
| `findings/F-VAL-031.md` | 286 | `assert!(generator.next(GROUP).await.unwrap.is_some); // FAILS TODAY` |
| `findings/F-VAL-032.md` | 40 | `.find(\|epoch\| epoch.group.id == event.gid)` |
| `findings/F-VAL-032.md` | 110 | `let signers = participating_epoch.group.participants.clone;` |
| `findings/F-VAL-032.md` | 111 | `let deadline = block.saturating_add(self.config.signing_timeout.get);` |
| `findings/F-XC-051.md` | 160 | `group.participants.register(msg.sender, poap);` |
| `findings/F-VAL-002.md` | 248 | `let cxy = x.ecdh(&y.public_key, fx);         // published by X` |
| `findings/F-VAL-002.md` | 249 | `let cyx = y.ecdh(&x.public_key, fy);         // published by Y` |
| `findings/F-CORE-002.md` | 251 | `let fetch = if retries < self.config.block_single_query_retry_count.get {` |
| `findings/F-CORE-002.md` | 252 | `if self.config.use_client_filtering { Fetch::ClientFiltered { block_hash, logs_bloom } }` |
| `../state/coverage-logs.md#r6` | 78 | `grep -n "fn fetch_logs\\ | fn decode_and_sort\\ | address(self.addresses\\ | address: log.inner.address\\ | macro_rules! watcher_events\\ | fn decode_log\\ | decode_raw_log\\ | SELECTORS" crates/core/src/index/events.rs \| head -30` |
| `../state/coverage-logs.md#r2` | 81 | `grep -n "observability::init\\ | tokio::main\\ | fn main" crates/{validator,sentinel,sentinel-engine}/src/main.rs` |
| `../state/coverage-logs.md#r10` | 116 | `grep -rn --include='*.rs' 'reqwest::Client\\ | ClientBuilder\\ | Client::new\\ | Client::builder' crates/` |
