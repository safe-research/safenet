# F-VAL-064 The shipped deployment cannot detect a halted validator: fatal exits return code 0, `/health` is unreachable by default, and the container runs as root

| Field | Value |
| --- | --- |
| Status | QA-done |
| Crate and module | validator, main.rs + Dockerfile + validator.sample.toml |
| Location | crates/validator/src/main.rs:95-99, crates/validator/Dockerfile:24-37 (related: crates/core/src/driver.rs:186-197, crates/core/src/observability/mod.rs:23-36, crates/validator/validator.sample.toml:61-64) |
| Severity | Medium / Medium |
| Certainty | 68% (Critic C-VAL-B; QA may raise) |
| Assumptions involved | A1, A5 |
| Tags | config, dos |

## Claim

Three independently minor gaps in the validator's deployment surface compose into one operationally significant one: **a validator that has fatally stopped is indistinguishable from a validator that shut down cleanly, from every angle the shipped artefacts expose.**

- `Driver::run` returns ``. It logs `error!` and breaks its loop on an unrecoverable watcher error — including `ExceededMaxReorgDepth`, the "deliberate exit" that A5 makes the designed response to a deep reorg — and on any unrecoverable driver error. `main` then does `driver.run.await;` followed by `Ok()`, so the process exits with status **0** in exactly the cases where it must not. Under Kubernetes `restartPolicy: OnFailure` (the natural choice for a stateful, single-writer service) the pod is marked `Completed` and never restarted; under systemd `Restart=on-failure` the unit stays stopped; any alert keyed on a non-zero exit sees a clean shutdown. `main` has no way to do better because `run` does not report why it returned.
- The one liveness surface, `/health` on the metrics listener, binds to `127.0.0.1:0` by default — an ephemeral port on loopback, so neither the port nor the address is knowable to a container probe. `validator.sample.toml` ships the `metrics_address` override commented out, and the `Dockerfile` declares no `EXPOSE` and no `HEALTHCHECK`. A validator following the sample has no reachable health endpoint at all.
- The runtime image declares no `USER`, so the binary runs as **root** inside the container. This one is not covered by A1: A1 says the operator provisions secrets honestly, not that the process should hold more privilege than it needs. The validator's job is to read one TOML file and read/write one SQLite file; running it as uid 0 means any memory-safety or logic escape in the process (or in `libsqlite3`, the one C dependency it links) starts from root, and a bind-mounted data directory ends up root-owned on the host.

Two smaller image-hygiene points in the same file: both base images are floating tags (`rust:1-slim`, `debian:trixie-slim`) with no digest pin, so the image is not reproducible and a compromised upstream tag is trusted implicitly despite `--locked` pinning the Rust dependencies; and there is no `.dockerignore` anywhere in the repository while the builder does `COPY crates/ crates/`, so a real `validator.toml` left beside the source is copied into the build context and the builder layer. It does not reach the final image (the runtime stage copies only the binary), but it does reach any cache or builder artefact that is pushed or shared.

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The runtime image declares no `USER`, so the binary runs as root | E2 | `crates/validator/Dockerfile:24-37` | `FROM debian:trixie-slim AS runner`<br>`WORKDIR /usr/src/app`<br>``<br>`RUN apt-get update && apt-get install -y --no-install-recommends \`<br>`	ca-certificates \`<br>`	libsqlite3-0 \`<br>`	&& rm -rf /var/lib/apt/lists/*`<br>``<br>`COPY --from=builder /usr/src/app/target/release/validator ./validator`<br>``<br>`# `ENTRYPOINT`(not`CMD`) so that a Kubernetes/`podman kube play` pod spec's`<br>`# `args:`(e.g.`--config-file=...`) appends to the binary instead of`<br>`# replacing it.`<br>`ENTRYPOINT ["./validator"]` |
| 2 | There is no `.dockerignore` at the repository root, and the builder copies the whole `crates/` tree | E2 | `crates/validator/Dockerfile:15-18` | `COPY Cargo.toml Cargo.lock ./`<br>`COPY crates/ crates/`<br>``<br>`RUN cargo build --release --locked --package validator` |
| 3 | Both base images are floating tags with no digest pin | E2 | `crates/validator/Dockerfile:5 and :24` | `FROM rust:1-slim AS builder`<br>`FROM debian:trixie-slim AS runner` |
| 4 | The driver logs and breaks out of its loop on any unrecoverable error, returning normally | E2 | `crates/core/src/driver.rs:186-197` | `            let result = match input {`<br>`                Err(err) => {`<br>`                    tracing::error!(?err, "unrecoverable watcher error; exiting");`<br>`                    break;`<br>`                }`<br>`                Ok(input) => self.update(input).await,`<br>`            };`<br>`            if let Err(err) = result {`<br>`                tracing::error!(?err, "unrecoverable driver error; exiting");`<br>`                break;`<br>`            }`<br>`        }` |
| 5 | and `main` discards that outcome and returns `Ok()`, so the process exit code is 0 | E2 | `crates/validator/src/main.rs:95-99` | `    tracing::info!("starting validator service");`<br>`    driver.run.await;`<br>``<br>`    Ok()`<br>`}` |
| 6 | The only liveness surface is the metrics listener's `/health`, which defaults to an ephemeral port on loopback and so is unreachable from a container probe unless the operator overrides it | E2 | `crates/core/src/observability/mod.rs:23-26` | `    /// The address the Prometheus metrics HTTP listener binds to (see`<br>`    /// [`metrics::serve`]). Defaults to `127.0.0.1:0`, which picks an ephemeral`<br>`    /// port on the loopback interface.`<br>`    pub metrics_address: SocketAddr,` |
| 7 | and the sample config ships that override commented out | E2 | `crates/validator/validator.sample.toml:61-64` | `# Optional: address the Prometheus metrics HTTP listener binds to. Defaults`<br>`# to an ephemeral port on loopback; set this to expose metrics outside a`<br>`# container (see the validator handbook's "Logging and Metrics" section).`<br>`# metrics_address = "0.0.0.0:3555"` |

## Trigger

- **Exit code:** any condition that breaks the driver loop. The designed one is a reorg deeper than `max_reorg_depth` (default 5), which `next_input` deliberately refuses to retry and returns to `run`, which logs "unrecoverable watcher error; exiting" and returns — process status 0. A5 makes this an expected event, not a hypothetical.
- **Health:** run the validator from `validator.sample.toml` in the shipped image and try to probe it. `metrics_address` is `127.0.0.1:0`, so there is no fixed port to probe and nothing outside the container's loopback can reach it.
- **Root:** `podman run` / `kubectl apply` with the shipped image and no `securityContext`. Verified by the absence of any `USER` line in `crates/validator/Dockerfile`.

## Considered and rejected

- **"A1 (trusted operator) makes container hardening out of scope."** A1 is about the honesty and provenance of secrets and config, and the brief's exclusion is specifically "the key is on disk". Running as root is neither: it is a privilege the process does not need, in an image the repository ships, and it is in a file that PROMPT.md Section 4 lists as in scope (`crates/*/Dockerfile`).
- **"This is CORE-H3 restated, and CORE-H3 belongs to R2."** The exit-code mechanism is CORE-H3 and R2 owns `driver.rs`. What is filed here is the validator's own half — `main.rs:95-99` is an assigned file — and, more importantly, the _composition_ with the two deployment artefacts that are also mine: it is the combination of exit-0, an unreachable `/health`, and no `HEALTHCHECK` that leaves the operator with nothing. Merge the exit-code claim with R2's if both survive; the health and privilege claims stand independently.
- **"A supervisor with `restartPolicy: Always` restarts it anyway, so the exit code is cosmetic."** Only if the operator chose `Always`. For a service with a durable single-writer SQLite database and a documented deliberate-exit path, `OnFailure` is a defensible and common choice, and it is precisely the choice this bug defeats. Restarting also does not fix the deep-reorg case (the process re-anchors by block number with no hash check, CORE-H1), so silently restarting is not obviously better than silently stopping — the operator needs to _know_, and neither outcome tells them.
- **"The builder stage could bake a real key into the image."** Checked and rejected as an image-content issue: the runtime stage is `FROM debian:trixie-slim` and copies only `target/release/validator`, so nothing from the builder's context reaches the published image. It remains a build-context hygiene point, not a secret-in-image finding.
- **Checked and clean:** `ENTRYPOINT ["./validator"]` with no `CMD` means a missing `--config-file` falls back to `validator.toml` in `/usr/src/app`, which does not exist, so `Config::load` returns an IO error and `main` returns `Err` — that path _does_ exit non-zero, and the `ENTRYPOINT`-not-`CMD` choice is deliberate and correct for the `args:`-appending pod spec the comment describes.

## Remediation options

1. Make `Driver::run` report its outcome — `run(self) -> Result<, Error>` or an enum distinguishing `Shutdown` from `Fatal(err)` — and have each `main` propagate it, or at minimum call `std::process::exit(1)` after a fatal return. One line per binary once the core signature changes. (Cross-crate: the sentinel's `main.rs` ends identically.)
2. Give the image a non-root user: `RUN useradd --system --uid 10001 safenet` in the runtime stage, `USER 10001`, and document that the data directory must be writable by that uid. Optionally `WORKDIR /var/lib/safenet` so the default config path is in a mounted directory rather than the image.
3. Default `metrics_address` to `0.0.0.0:3555` for containerised use, or at least uncomment it in `validator.sample.toml` with an `EXPOSE 3555` and a `HEALTHCHECK CMD` in the Dockerfile so the shipped artefacts form a probeable unit. Then make `/health` reflect driver liveness (last processed block advancing) rather than returning a constant `OK`, so a stalled-but-alive validator — the CORE-H8 / M9 retry-forever case — is also detectable.
4. Pin base images by digest (`rust:1-slim@sha256:…`) and add a repository-root `.dockerignore` covering `*.toml` configs, `*.db`, `target/` and `.git/`.

Tests to add: none of this is unit-testable in-process, but the integration workflow that already runs the validator happy path could assert a non-zero exit after injecting a deep reorg, which is the same scenario `integration.yml`'s deep-reorg regression already sets up.

## Trail

- Reviewer R6: drafted while reading the two non-Rust files in my scope. Every cited line re-opened; the absence of `USER`, `EXPOSE`, `HEALTHCHECK` and `.dockerignore` was confirmed by grep over all three Dockerfiles and an `ls` at the repository root. Self-estimate 90% on the mechanism of all three claims (they are direct reads of shipped files); the Medium severity rests on a judgement about deployment practice that the team can overrule with what their actual manifests do.

## Critic (C-VAL-B)

Derived from `main.rs:32-99`, `crates/validator/Dockerfile` in full, `core/driver.rs:170-198`, `core/observability/mod.rs:15-63` and `validator.sample.toml:57-64` before reading the Claim.

### Per-claim verdicts

Three of four **Supported**; one is **Unsupported → `H`**.

- **Exit code — Supported.** `Driver::run` returns ``, logging `error!` and `break`ing on both the unrecoverable-watcher and unrecoverable-driver arms (`core/driver.rs:186-196`), and `main` does `driver.run.await;` then `Ok()` (`main.rs:96-98`). Process status 0 on a fatal stop. The `ExceededMaxReorgDepth` path really is routed to `run` rather than retried (`core/driver.rs:211-215`), so A5's designed exit is one of the cases that returns 0.
- **Health — Supported.** `observability::Config::default` is `metrics_address: SocketAddr::from((Ipv4Addr::LOCALHOST, 0))` (`core/observability/mod.rs:33`), `validator.sample.toml:61-64` ships the override commented out, and the Dockerfile declares no `EXPOSE` and no `HEALTHCHECK`. An ephemeral loopback port is not probeable from outside the container.
- **Root — Supported.** No `USER` line exists in the file; the runtime stage is `FROM debian:trixie-slim`, `COPY --from=builder ... ./validator`, `ENTRYPOINT ["./validator"]` (`Dockerfile:24-37`). Floating base tags confirmed at `:5` and `:24`.
- **`.dockerignore` — Unsupported, marked `H`.** The Claim states "there is no `.dockerignore` anywhere in the repository", and the Trail says the absence "was confirmed by grep over all three Dockerfiles and an `ls` at the repository root". The checkout contradicts it: `crates/validator/Dockerfile.dockerignore` exists (42 bytes, 4 lines) and contains

  ```
  /**
  !/Cargo.toml
  !/Cargo.lock
  !/crates/**
  ```

  as do `crates/sentinel/Dockerfile.dockerignore`, `crates/sentinel-engine/Dockerfile.dockerignore` and `contracts/Dockerfile.dockerignore`. Phase 0 recorded all three of the crate ones (`state/baseline.md` §4), so this was independently knowable. A per-Dockerfile `<name>.dockerignore` is honoured by BuildKit **in preference to** a root `.dockerignore`, so the finding's basis row 2 is wrong on the fact and its remediation 4 ("add a repository-root `.dockerignore`") would be a no-op: BuildKit would ignore the new file entirely while the existing one keeps deciding the context. The residual point that survives is narrower and should replace the current text — the allow-list re-includes `/crates/**` wholesale, so a real `validator.toml` or `*.db` left _inside_ `crates/validator/` does reach the builder context, while one at the repository root does not.

### Finding verdict

**Confirmed — 68%.** The three surviving claims are direct reads of shipped files with no inference in them at all; the trigger for each is "run the shipped artefacts as shipped". The number is held down by the `H` above and by the fact that the exit-code and health claims describe operational consequences under deployment manifests that are not in this repository — the reviewer says so themselves in the Trail, which is the right posture.

**Severity: Medium (unchanged), and I checked it both ways as the brief asked.** Not High: nothing here is reachable by an attacker, nothing loses key material, and every consequence is mediated by a deployment choice the operator makes. Not Low: exit code 0 on a fatal stop is a genuine correctness defect in the process contract — it converts A5's _designed_ deep-reorg exit into a silent permanent outage under the two most common supervisors — and unlike the other two it cannot be worked around by configuration, because `run` does not tell `main` why it returned. The root-user point on its own would be Low (A1 grants an honest operator and a hardened `securityContext` is a one-line manifest change); it is included here as one of three and does not carry the rating.

**Remediation note.** The load-bearing fix is to make `Driver::run` return `Result<, Error>` (or an enum distinguishing "shutdown signal" from "fatal") so `main` can `std::process::exit(1)`; everything else in the finding is a deployment-manifest change the team can make without touching Rust. Remediation 4 must be rewritten per the `H` above.

### Addendum (C-VAL-B) — re-derived build-context risk, and deferral to F-XC-004

C-XC independently flagged the same `H` I recorded above; we agree on the fact and on the four file paths. Re-deriving the residual risk properly, since the finding must not simply lose the concern:

`crates/validator/Dockerfile.dockerignore` is `/**` followed by `!/Cargo.toml`, `!/Cargo.lock`, `!/crates/**`. So the context is _deny everything, then re-admit `crates/` wholesale_. There is no pattern excluding `*.toml`, `*.db`, `*.sqlite` or `target/` **under** `crates/`. The builder does `COPY crates/ crates/` (`Dockerfile:16`). Therefore:

- a real `validator.toml`, or the SQLite file that `docs/validator-handbook.md:58` says must be "treat[ed] ... as containing secret keys", left anywhere under `crates/` **is** copied into the build context and baked into the builder layer;
- the same file at the repository root is **not**, which is what the ignore file does buy;
- adding a root `.dockerignore` (the finding's remediation 4) would be inert, because BuildKit prefers `<dockerfile>.dockerignore` when it exists and ignores the root file entirely.

**Corrected claim**, replacing basis row 2 and the second half of the Claim's last paragraph: _the per-Dockerfile ignore file's allow-list re-admits `/crates/**` without excluding configuration or database files under it._ **Corrected remediation 4**: add `crates/**/*.toml`, `crates/**/*.db`, `crates/**/*.sqlite*` and `**/target` as further exclusions **inside the existing `crates/validator/Dockerfile.dockerignore`** (and its two siblings), and keep `!crates/validator/validator.sample.toml` if the sample is wanted in the context. Do not add a root `.dockerignore`.

**Deferral.** Per C-XC, **F-XC-004 is canonical for image hardening** — the missing `USER`, the floating `rust:1-slim` / `debian:trixie-slim` tags and the absent digest pins. Those three should be read from F-XC-004; what remains validator-specific in this file, and what it should be read for, is the **exit-code-0 defect** (`main.rs:96-98` over `core/driver.rs:186-196`) and the **unreachable `/health`** default (`core/observability/mod.rs:33` plus the commented-out `validator.sample.toml:61-64`). My verdict and severity are unchanged: **Confirmed, 68%, Medium**, carried by the exit-code defect, which is the only one of the five that cannot be fixed from a deployment manifest.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **68%**; severity Medium unchanged. No PoC directory: none of the three claims is unit-testable in-process, which the finding already says, and writing a Dockerfile-linting harness that has never been run would add nothing.

### What would be run, and what it would show

The exit-code claim is the only one with a mechanical check, and it is a shell one-liner rather than a Rust test:

```sh
# with a toolchain, and a database whose snapshot tip is older than max_reorg_depth
cargo run -p validator -- --config-file validator.toml ; echo "exit=$?"
```

`Driver::run` returns `` (`crates/core/src/driver.rs:186-197`), so `main` returns `Ok()` and the process exits 0 whether the driver shut down cleanly or bailed out on a deep reorg. Asserting `exit != 0` after injecting a reorg deeper than `max_reorg_depth` is the acceptance test for remediation option 1, and the finding is right that the existing deep-reorg scenario in `integration.yml` already sets up the state it needs — extending that workflow is cheaper than building anything new.

The other two claims — `/health` unreachable by default, container runs as root — are direct reads of `crates/validator/validator.sample.toml:61-64` and `crates/validator/Dockerfile:24-37`. They need no run and they are already `E2`.

### Remediation check

**Option 1 (make `Driver::run` report its outcome) is sound and is the load-bearing fix**, exactly as C-VAL-B says. Two notes for whoever implements it:

- It is a **cross-crate** change: `Driver::run`'s signature is in `safenet-core` and both binaries end identically, so the sentinel's `main.rs` must move with it. Scope it as a core change with two call-site updates, not as a validator change.
- The distinction that matters is _shutdown signal_ versus _fatal_, not `Result` versus ``. A plain `Result<, Error>` invites `main` to `?` it and print a message, which is right; but a clean SIGTERM shutdown must not become exit 1, or every rolling restart looks like a crash. The enum the option offers as an alternative is the better shape.

**Option 2 (non-root user) is sound and is the smallest of the four.** One thing to specify: the `USER 10001` line must come _after_ any `COPY` that needs root, and the data directory bind-mounted at runtime must be writable by that uid — which is a deployment-manifest change the image cannot make for itself. Say so, or the first deployment after the change fails to open its database and the change is reverted.

**Option 3 has two halves and only the second is a real fix.** Defaulting `metrics_address` and adding `EXPOSE`/`HEALTHCHECK` makes the artefacts probeable, which is worth doing. But making `/health` "reflect driver liveness (last processed block advancing)" is the part that matters, and the option understates it: today `/health` returns a constant, so a validator that is stalled in every way this audit found — F-VAL-004's genesis stall, F-VAL-030's phantom chunk, F-VAL-031's dead worker — answers OK. A liveness probe that reads the snapshot tip and fails when it has not advanced in `n` blocks is the single change that would surface four separate findings operationally, and it should be prioritised above options 2 and 4.

**Option 4 must be rewritten**, per the `H` the Critic recorded above and C-XC's independent finding of the same fact. Whatever survives of it should be stated against the four `Dockerfile.dockerignore` paths that actually exist, and the base-image digest pinning — which is sound and unaffected by the `H` — should be split out so it is not lost with the rest.

**Severity.** Medium is right and I agree with C-VAL-B's reasoning in both directions. The point worth carrying into the report is the one that distinguishes claim 1 from the other two: exit code 0 on a fatal stop cannot be worked around by configuration, because `run` does not tell `main` why it returned. The root user and the unreachable `/health` are both one-line deployment fixes; this one is not.

## Scope note (run 2)

`crates/sentinel-engine` was placed out of scope after this finding was written (PROMPT.md Section 11); the engine-specific parts above are retained as run-1 history and are not part of the current audit's claims. The non-engine parts stand.
