# QA-XC note — question 21 of UNRESOLVED-DEPENDENCY-QUESTIONS.md is **answered**

`F-CORE-038` belongs to another QA agent; this file adds one result and claims nothing else about the finding. Named `README-QA-XC.md` rather than `README.md` so it cannot collide with that agent's own write.

## What was run, and what it does not prove

C-CORE-B flagged that the reviewer declined to re-derive the HKDF reference vector at `crates/core/src/kdf.rs:36-46`, and the Coverage Critic carried it as question 21 of the toolchain-blocked list. It does **not** need a Rust toolchain — `python3` is present on this machine, and the vector's own doc comment says it was produced with Python `hmac`/`hashlib` in the first place.

```sh
python3 hkdf_reference_vector.py     # output saved to hkdf_reference_vector.out
```

Executed at commit `2893917`. Output:

```
de66ad87d39718318f7ec36177e9e2286b5c0ade3dc0de22b65e9ee55ccaab0d
True
```

RFC 5869 HKDF-SHA256 with `salt = b"safenet-sentinel-reveal-salt"`, `ikm = b"top secret key material"`, `info = b"request-1"`, `L = 32` reproduces the literal in the test **exactly**. The extract-then-expand structure the script implements is the standard one: `PRK = HMAC(salt, ikm)`, `OKM = HMAC(PRK, info ‖ 0x01)[..32]`, which matches how `derive_key` uses `Hkdf::<Sha256>::new(Some(domain), ikm)` with `domain` as the salt and `message` as the info parts (`crates/core/src/kdf.rs:19-27`).

**What this proves:** the vector is arithmetically correct and independently reproducible, so it is not a value copied from a previous (possibly wrong) run of the code under test. C-CORE-B's concern is closed.

**What it does not prove:** that the Rust in this checkout produces it. That still needs `cargo test -p safenet-core kdf::tests::derive_key_matches_reference_vector`, which is an ordinary green-suite check rather than an open question — it exercises `hkdf 0.13` and `sha2 0.11` (`Cargo.lock:2465-2492`, `:4455-4476`), which nothing here can run.

One residual worth stating, because it is a real property of the design and not of the vector: `expand_multi_info` feeds the `message` parts to HMAC incrementally without a separator, so `derive_key(ikm, d, &[b"ab", b"c"])` and `derive_key(ikm, d, &[b"a", b"bc"])` collide. The finding already records that the only in-tree caller passes exactly one part (`crates/core/src/tx/signer.rs:59-64`), so it is unreachable today; it is a hazard for the next caller, not a defect.
