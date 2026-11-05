# F3-CORE-031 An RPC URL with an unsupported scheme is printed in full, API key included, when the service starts

| Field | Value |
| --- | --- |
| Status | Final: duplicate of F3-XC-130 (follows its rating) |
| Crate and module | core, provider/mod.rs |
| Location | crates/core/src/provider/mod.rs:145-149 (related: crates/validator/src/main.rs:33, :45; crates/sentinel/src/main.rs:33, :44; alloy-rpc-client 2.0.5 src/builtin.rs:248-277) |
| Severity | Informational / follows F3-XC-130 |
| Certainty | n/a (duplicate; see F3-XC-130) |
| Assumptions involved | A1 |
| Tags | config, known |

## Claim

`Provider::connect` hands the configured URL back to alloy as a string. alloy's connection-string parser tries HTTP, then WebSocket, then IPC, and keeps the last error; the IPC attempt formats the whole input into its message. So a URL that the `url` crate accepts but whose scheme is not `http`, `https`, `ws` or `wss` (a typo such as `htps://`, or a value injected through `${NAME}`) ends the process with the full URL, path and query included, on stderr. RPC providers put the API key there. This is the residual gap #1046 lists as "a malformed RPC URL is still quoted in its own startup error (the message comes from the `url` crate)" (`55d811e` message), hence `known`. What this adds: the quote does not come from the `url` crate (its `ParseError` messages are fixed strings, so a URL it rejects is never quoted); it comes from alloy's IPC fallback, only for well-formed URLs, and Safenet can close it itself.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The parsed `Url` is turned back into a string and parsed again by alloy, outside the observability layer that strips URLs | E2 | crates/core/src/provider/mod.rs:145-149 | `pub async fn connect(url: &Url) -> Result<Self, TransportError> {` / `let client = ClientBuilder::default()` / `.layer(ObservabilityLayer)` / `.connect(url.as_str())` / `.await?;` |
| 2 | alloy keeps the last failure of HTTP, WS and IPC parsing | E2 | alloy-rpc-client-2.0.5/src/builtin.rs:266-276 (registry source, version pinned in Cargo.lock) | `fn from_str(s: &str) -> Result<Self, Self::Err> {` / `...` / `let res = res.or_else(\|_\| Self::try_as_http(s));` / `...` / `let res = res.or_else(\|_\| Self::try_as_ws(s));` / `...` / `let res = res.or_else(\|_\| Self::try_as_ipc(s));` / `res` |
| 3 | The IPC attempt quotes the input | E2 | alloy-rpc-client-2.0.5/src/builtin.rs:248-256 | `pub fn try_as_ipc(s: &str) -> Result<Self, TransportError> {` / `let s = s.strip_prefix("file://").or_else(\|\| s.strip_prefix("ipc://")).unwrap_or(s);` / `...` / `let msg = format!("failed to read IPC path {}: {e}", path.display());` |
| 4 | `main` returns the error, which Rust prints with `Debug` on exit | E2 | crates/validator/src/main.rs:33, :45 (same in crates/sentinel/src/main.rs:33, :44) | `async fn main() -> Result<(), Box<dyn Error>> {` / `let provider = Provider::connect(&config.rpc).await?;` |
| 5 | Executed: the validator binary at `361aaf3` with `rpc = "htps://gnosis.rpc.example.invalid/v3/R2-FAKE-API-KEY-0123456789"` prints the key and exits 1, without any network access | E1 | rust-audit/state/run3/logs/R2-startup-config-checks.txt | `Error: Transport(Custom("failed to read IPC path htps://gnosis.rpc.example.invalid/v3/R2-FAKE-API-KEY-0123456789: No such file or directory (os error 2)"))` |

## Trigger

An operator writes `rpc = "htps://<provider>/v3/<API_KEY>"` (or sets `RPC_URL` to such a value and uses `rpc = "${RPC_URL}"`), on either service. The config parses, `Provider::connect` fails in alloy's parser, and `main` prints `Error: Transport(Custom("failed to read IPC path htps://<provider>/v3/<API_KEY>: ..."))` to stderr, which container runtimes keep in the service log.

## Considered and rejected

- `without_url` (crates/core/src/provider/mod.rs:124-134) does not apply: it only maps errors returned through `ObservabilityTransport`, and this error is raised before the transport exists.
- A URL the `url` crate rejects is not quoted: `from_str_with_env` maps `url::ParseError` with `de::Error::custom` (crates/core/src/serialization.rs:69), and `url::ParseError` messages carry no input; `config::ParseError` drops the document (crates/core/src/config.rs:37).
- Related, also listed in #1046 and not filed here: alloy's `ReqwestTransport` debug span records `url = %this.url` (alloy-transport-http-2.0.5/src/reqwest_transport.rs:92). #1046 says this needs `alloy_transport_http=debug`; a global `log_filter = "debug"` or `"trace"` (the devnet script writes `log_filter = "trace"`, scripts/run_devnet.sh:377) turns it on as well. Recorded as an observation in the R2 coverage log.
- Severity stays Informational: the operator's own typo, the output goes to the operator's own logs (A1), and the service does not start, so the mistake is noticed at once.
- Pre-existing: `fe9e84c` already connects with `.connect(url.as_str())` (`git show fe9e84c:crates/core/src/provider/mod.rs`, line 132).

## Remediation options

1. Reject schemes other than `http`, `https`, `ws`, `wss` when the config is parsed (a small newtype around `Url` with a fixed error message), so the bad value never reaches alloy.
2. Build the transport from the parsed `Url` directly (for HTTP, `alloy::transports::http::Http::new(url)` boxed into the client) instead of round-tripping through the connection-string parser; keep the string path only for WS if needed.
3. Map the `connect` error through a URL-free message before returning it.

Tests to add: a config test asserting that `rpc = "htps://host/KEY"` fails without `KEY` in `Display` or `Debug` of the error, like `errors_do_not_contain_the_configuration` in crates/core/src/config.rs. No code is committed.

## Trail

- 2026-10-08 Reviewer R2: drafted, self-estimate 95% that the leak happens as described; Informational and `known` because #1046 already accepts a startup-error leak of the RPC URL, though with a different cause.

## Critic (C3-CORE, 2026-10-08)

| Row | Verdict | Note |
| --- | --- | --- |
| 1 | Supported | `crates/core/src/provider/mod.rs:145-149`. |
| 2 | Supported | alloy-rpc-client 2.0.5 `src/builtin.rs:266-276` (registry source at the locked version). |
| 3 | Supported | `builtin.rs:248-256`. |
| 4 | Supported | `crates/validator/src/main.rs:33, 45`; `crates/sentinel/src/main.rs:33, 44`. |
| 5 | Supported | `rust-audit/state/run3/logs/R2-startup-config-checks.txt:8, 16`. |

Finding verdict: Confirmed. Severity: Informational to Informational, tag `known`. Certainty: 90, band 90 to 100 (E1, Critic Confirmed). Duplicate: duplicate of F3-XC-130 (same path, same trigger; F3-XC-130 adds the `${NAME}` case, an `https` control run and the feature check). Only this file adds the pointer to alloy's `ReqwestTransport` debug span (`alloy-transport-http-2.0.5/src/reqwest_transport.rs:92`), which #1046 already lists as not covered, so it stays a known observation.

QA recipe: see F3-XC-130.

## Practicality (PC3, 2026-10-09)

Duplicate of `F3-XC-130` (C3-CORE); it follows that row: theoretical (operator typo, own logs), Informational, tag `known`. See the PC3 practicality section in `F3-XC-130.md`.

## Confidence (CA3, 2026-10-09)

Duplicate of `F3-XC-130`. No separate score: this row follows `F3-XC-130`'s final status, severity and certainty (see the CA3 confidence section there).

## Reconciliation (run 3)

| Field | Value |
| --- | --- |
| Relation | Duplicate of `F3-XC-130` (run 3), which is NEW |
| Canonical | `F3-XC-130`. This file stays this run as independent evidence and follows the canonical row; listed in the ledger's fold index (section 3.1) |
| Source | Ledger: [`report/RECONCILIATION.md`](../report/RECONCILIATION.md) sections 2 and 3 |
