# PoC — F-XC-008 item 1 (the CoW client has no timeout)

**Never compiled, never run.** No Rust toolchain (A9 FALSE). Commit `2893917`.

Also settles, as a side effect, questions 8 and 10 of `UNRESOLVED-DEPENDENCY-QUESTIONS.md` (reqwest 0.13.4's default request timeout; whether an un-timed request can hang), which `F-CORE-011`, `F-CORE-039`, `F-ENG-005` and `F-ENG-043` all lean on as a class `I` premise.

## Install and run

Append `append-to-crates-sentinel-engine-src-checkers-cow.rs` inside the existing `mod tests` block at the end of `crates/sentinel-engine/src/checkers/cow.rs`. `ReqwestOrderApi` (`cow.rs:184-186`) is private to that module, so the test has to live there — which is a feature: it drives the real production type through the real `fetch_order`, not a synthetic `reqwest` call that might not share the client's configuration.

```sh
cargo test -p sentinel-engine qa_xc_008 -- --ignored --nocapture
```

Both tests are `#[ignore]`d because they spend five seconds of wall clock each. That is also why they should not go into CI as written; if the team wants them in CI, drop the budget to 1.5 s, which is still far above any plausible internal cap.

## Reading the result

| Outcome | Meaning | Action |
| --- | --- | --- |
| `…has_no_timeout` **passes** and `…a_configured_timeout…` **passes** | Confirmed: a wedged CoW endpoint parks the check indefinitely, and the one-line remediation bounds it. | `F-XC-008` item 1, `F-ENG-005` and `F-ENG-043` move to `E1`. `F-ENG-005` is the canonical home for the severity call. |
| `…has_no_timeout` **fails** | reqwest or hyper applies an internal cap. | Re-score downwards: the mechanism is a delay, not a stall. Record the observed duration — it is the answer to question 10 and it changes four findings at once. |
| `…a_configured_timeout…` **fails** | `.timeout`/`.connect_timeout` did not bound the call. | The proposed remediation is wrong. Investigate before the fix ships; the fallback is a `tokio::time::timeout` wrapper at the call site, which cannot fail to bound it. |

## Fixtures

A loopback `TcpListener` that accepts and never responds — no network, no third party, no attacker. This models an ordinary availability event at `api.cow.fi`, which is the trigger `F-XC-008` claims, rather than a partition (the OS would eventually reset that) or a DNS failure (which returns promptly).

`order_uid` is 56 bytes of `0xab`, matching the existing `ORDER_UID` fixture in the same test module (`cow.rs:650`). The URL path built by `fetch_order` is `{base}/api/v1/orders/{order_uid}`; the server never reads it.

## Remediation check (QA-XC)

`F-XC-008` remediation 1 (build the client with `.timeout(..)`/`.connect_timeout(..)`) is **sound**, and the seam already exists — `CowChecker::with_client` (`cow.rs:234`) was written for exactly this and is used by the tests, so the change is one expression in `CowChecker::new`. The second test here is the proof that the fix works, not just that the bug exists; a remediation check that only demonstrates the defect is half a check.

Two cautions:

- A timeout converts a stall into an `OrderLookupError`, which `CowChecker` turns into `Verdict::Abstain`. That is the right failure direction here (`cow.rs:296` recomputes the EIP-712 order UID, so a CoW response cannot forge a verdict either way), but it means the fix trades a stall for an abstention, and under `F-XC-010` an abstention is invisible. The two findings should be fixed together or the improvement will not be observable.
- `F-XC-008` remediation 2's `no_proxy` caveat is well taken and should **not** be hard-coded: an operator legitimately behind a corporate proxy would lose all CoW lookups. Prefer documenting the proxy environment variables the client honours over disabling them.

`F-ENG-043`'s Critic adds a point worth carrying into the fix: the outbound base URL is selected from `transaction.chain_id` (`cow.rs:112`, `:422-427`), not from the operator's configured provider, so the set of third parties contacted is proposer-selected. A timeout bounds the damage from that; it does not remove it.
