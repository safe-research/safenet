# Questions settled by V-XC in Phase 5

Author: **V-XC**. Own file, deliberately not an edit of the shared `UNRESOLVED-DEPENDENCY-QUESTIONS.md` — that file was already lost once to a concurrent write (`rust-audit/state/STATE.md`, "Data loss and recovery") and other verification agents are running against tracked files right now. The manager should merge these answers back.

Numbering follows `rust-audit/poc/UNRESOLVED-DEPENDENCY-QUESTIONS.md`.

Environment: cargo/rustc 1.98.1, `stable-aarch64-unknown-linux-gnu`, `cargo-audit` 0.22.2. **A9 is now TRUE. A8 is still FALSE** — no Foundry/anvil, no test-vector corpus.

---

## Q3 — `#[serde(deny_unknown_fields)]` together with `#[serde(flatten)]`

**ANSWERED: it works. All six of QA-XC's tests pass, on both crates.** Unknown top-level keys, `use_client_filterring` inside `[index]` (the A4 switch), `max_reorg_dept` inside `[index]` (the A5 knob), and a mistyped `[observabilty]` table are all **rejected**. The validator's `#[serde(default, flatten)]` and the sentinel's bare `#[serde(flatten)]` behave identically. Full transcript and consequences: `rust-audit/findings/F-XC-003.md`, Verification section. **`F-XC-003` → Informational, 96%.** Also closes the top-level-key leg of `F-VAL-063`.

## Q4 — can an un-timed `reqwest` request hang indefinitely?

**ANSWERED: yes.** The real private `ReqwestOrderApi` built with `reqwest::Client::new` did not return within 5 s against a loopback listener that accepts and never answers. The control — the same code path through a client with `.timeout(500ms).connect_timeout(500ms)` — returned inside the budget with an error, so the proposed remediation works as written and needs no `tokio::time::timeout` wrapper at the call site. Full transcript: `rust-audit/findings/F-XC-008.md`, Verification section. **`F-XC-008` item 1 → E1, 94%.** This is the pivotal `I` leg for `F-ENG-005`, `F-ENG-043`, `F-CORE-011` and `F-CORE-039` as well; all four may now treat it as `E1`.

## Q6 — does alloy's ABI decoder pre-allocate a `Vec` from a declared array length before validating it?

**ANSWERED: yes it pre-allocates — and NO, it is not a memory-exhaustion vector. No finding. Record this in the report's rejected list so nobody re-raises it.**

The mechanism is real and is in the source. `alloy-sol-types-1.6.0/src/abi/token.rs:418-437`, `DynSeqToken::decode_from`:

```rust
let len = child.take_offset?;
let mut child = child.raw_child?;
let mut tokens = vec_try_with_capacity(len)?;      // <-- allocates from the DECLARED length
unsafe {
    T::decode_many_from(&mut child, &mut tokens.spare_capacity_mut()[..len])?;  // <-- validates AFTER
    tokens.set_len(len);
}
```

The reservation happens on the line before the payload is checked. But three properties together make it harmless, and I measured all of them. Test appended to `crates/sentinel-engine/src/engine/mod.rs`, run, reverted; a two-word payload (offset + length prefix, no elements at all) decoded as `Vec<U256>`, RSS and VSZ read from `/proc/self/statm` either side of each call:

```
2^20 (=1048576 elems, 32 MiB)        -> Err("ABI decoding failed: buffer overrun while deserializing")   dVSZ=0kB dRSS=192kB
2^24 (=16777216 elems, 512 MiB)      -> Err("ABI decoding failed: buffer overrun while deserializing")   dVSZ=0kB dRSS=0kB
2^26 (=67108864 elems, 2048 MiB)     -> Err("ABI decoding failed: buffer overrun while deserializing")   dVSZ=0kB dRSS=4kB
2^28 (=268435456 elems, 8192 MiB)    -> Err("ABI decoding failed: buffer overrun while deserializing")   dVSZ=0kB dRSS=0kB
2^30 (=1073741824 elems, 32768 MiB)  -> Err("memory allocation failed because the memory allocator returned an error")   dVSZ=0kB dRSS=0kB
2^32 (=4294967296 elems, 131072 MiB) -> Err("memory allocation failed because the memory allocator returned an error")   dVSZ=0kB dRSS=0kB
2^34                                  -> Err("memory allocation failed because the memory allocator returned an error")   dVSZ=0kB dRSS=0kB
2^40                                  -> Err("memory allocation failed because the memory allocator returned an error")   dVSZ=0kB dRSS=0kB
2^63                                  -> Err("memory allocation failed because the computed capacity exceeded the collection's maximum")   dVSZ=0kB dRSS=0kB
2^64                                  -> Err("type check failed for \"offset (usize)\" with data: 00...010000000000000000")   dVSZ=0kB dRSS=0kB
```

And the nested-dynamic form that `KeyGenCommitment.c` actually is (a tuple array — `crates/validator/src/bindings.rs:63-76`):

```
tuple-array 2^68 -> Err("type check failed for \"offset (usize)\"")   delta=0kB
```

Reading the three regimes:

1. **`>= 2^64`: rejected before the allocator is reached.** `take_offset` runs `utils::as_offset`, a `usize` range check, so the length word never becomes a capacity. **The `2^68` prefix the question proposed is the _least_ dangerous input of the set** — it is the one case that never touches the allocator at all.
2. **`2^30` … `2^63`: the allocation is attempted and _refused_.** `vec_try_with_capacity` is fallible (`try_reserve`), so it returns `Error::Reserve` rather than aborting the process. No abort, no OOM-killer, no panic — an ordinary decode error, which the callers already handle.
3. **`<= 2^28`: the reservation succeeds and is immediately dropped.** `decode_many_from` fails on the next line with "buffer overrun", the `Vec` is freed with `len == 0`, and **the pages are never touched** — hence `dVSZ = 0 kB` and `dRSS <= 192 kB` (allocator arena noise) even at an 8 GiB request. The cost is virtual address space for the duration of one function call, not resident memory.

Net: a single crafted log costs one transient `try_reserve` and returns an error. Nothing accumulates, nothing survives the call, and the peak is bounded by `try_reserve`'s own refusal. This does **not** support a new High, and it does not raise `F-VAL-001`, `F-VAL-003` or `F-XC-051` on the memory-exhaustion leg. It is the question's first outcome row in substance ("errors immediately, allocation flat") even though the literal mechanism is the second.

_One caveat I am not going to overstate:_ this was measured on a host with Linux default overcommit and a 64-bit address space. A deployment under a strict `ulimit -v`, a low `vm.overcommit_memory=2` ratio, or a container memory cgroup that counts reservations would move the refusal threshold down from 2^30 — it would still be an `Err`, not an OOM, but the _decode would start failing at smaller lengths_. That is a configuration note for the deployment handbooks, not a finding.

## Q7 — stock `release` profile defaults on the pinned toolchain

**ANSWERED: `overflow-checks = false`, `debug-assertions = false`. `F-XC-001` stands.** Also, from the same verbose build: `opt-level=3`, `strip=debuginfo`, `lto = false`, `codegen-units = 16`, `panic = "unwind"`. Full flag evidence: `rust-audit/findings/F-XC-001.md`, Verification section. **`F-XC-001` → E1, 93%.**

## Q8 — `cargo tree -d` vs the lockfile-derived duplicate list

Run by the manager: `rust-audit/state/logs/cargo-tree-dupes.txt`, 76 entries, `alloy-*` v1.6.0 under `alloy` v2.0.5. I did not re-run it. Note for `baseline.md` §6: the caveat this question was raised to close is **not** fully closed by `cargo tree -d` alone — `cargo tree -d` sees the resolved graph, but as Q9/`F-XC-011` show, the _lockfile_ additionally contains packages that are in no build graph at all (`quinn-proto`, `anyhow`, `bitcoin-io`, `bitcoin_hashes`, `chacha20`, `sqlx-mysql`, `sqlx-postgres`). Any list derived by parsing `Cargo.lock` will over-report against any list derived from `cargo tree`, and the difference is not an error in either — it is the optional-dependency set. That distinction should be stated in `baseline.md` §6 rather than reconciled away.

## Q9 — is there any advisory affecting any of the locked packages?

**ANSWERED in full: `rust-audit/findings/F-XC-011.md`** (new this phase). 4 vulnerabilities, 11 warnings; one reachable network surface; the 7.5 HIGH is not in any binary. Supersedes the advisory half of `F-XC-007`, whose original text correctly asserted nothing.

## Q15 — do `PrivateKeySigner` and k256's `SigningKey` zeroize on drop?

**ANSWERED: yes, and this codebase already handles the two copies it makes. This LOWERS the secret-at-rest severity language rather than raising it.** Settled by source reading, as the question anticipated; nothing was executed for it.

- `PrivateKeySigner = LocalSigner<k256::ecdsa::SigningKey>` (`alloy-signer-local-2.0.5/src/lib.rs:43`).
- `LocalSigner` itself has **no** `Drop` and no `ZeroizeOnDrop` impl (`lib.rs:85-93`; greps for `impl.*Drop for LocalSigner` and `ZeroizeOnDrop for LocalSigner` return nothing). It does not need one: it holds the credential **by value**, so dropping it drops the `SigningKey`.
- `ecdsa::SigningKey` **does** implement both — `impl<C> Drop for SigningKey<C>` (`ecdsa-0.16.9/src/signing.rs:380`) and `impl<C> ZeroizeOnDrop for SigningKey<C>` (`:485`) — and the inner `elliptic_curve::SecretKey` likewise (`elliptic-curve-0.13.8/src/secret_key.rs:318`, `:320`). So the scalar is wiped when the signer drops.
- `Signer` and `LocalSigner` are both `#[derive(Clone)]`, so a clone is a second copy of the scalar — but each copy carries the same `ZeroizeOnDrop` and is wiped on its own drop. Not a leak.
- **`to_bytes` is the one that leaves a plain copy**, exactly as the question suspected: `SigningKey::to_bytes` returns `FieldBytes<C>` **by value** (`signing.rs:106-108`), a `GenericArray<u8, 32>` with no zeroizing wrapper. **This codebase calls it once outside tests, and already cleans up after itself**: `crates/core/src/tx/signer.rs:60-63` does `let mut key = self.0.to_bytes; let derived = kdf::derive_key(key.as_slice, ...); key.0.zeroize;`. The `Deserialize` impl at `:88-91` does the same for the raw `B256` it parses the key from.

**Residual, and it is small.** Neither cleanup is panic-safe: `key.0.zeroize` is a statement, not a `Zeroizing<_>` wrapper or a drop guard, so if `kdf::derive_key` unwound, the 32-byte copy would be left on the stack. Under **A1** the operator and host are trusted and no finding becomes High on this. The cheap hardening is `zeroize::Zeroizing` around both, which makes the wipe run on the unwind path too and removes the need to keep the manual call correct as the function changes.

**Calibration for the report:** secret-at-rest findings should not be worded as though key material survives in freed memory by default. It does not — the library zeroizes, and the two places Safenet copies bytes out zeroize explicitly. The accurate residual claim is narrower: _the explicit wipes are not unwind-safe._

## Q19 — does the linker strip unused `sqlx-mysql` / `sqlx-postgres`?

**ANSWERED, and the question's premise is wrong one step earlier: they are never compiled at all, so there is nothing to strip.** `cargo tree -i` prints "nothing to print" for both; zero artefacts in `target/debug/deps`; `nm -C` finds zero matching symbols in all three release binaries after `cargo build --release --workspace --locked` (exit 0). `sqlx` 0.9's default features do not activate them, so `features = ["sqlite", "runtime-tokio"]` already excludes them with `default-features` left on. **`F-XC-007` item 2 is Refuted; its remediation 2 is a lockfile-only change, not a surface reduction.** Detail: `rust-audit/findings/F-XC-007.md`, Verification section.
