# F-VAL-031 A dead nonce-generation worker thread is never detected, logged, or restarted

| Field | Value |
| --- | --- |
| Status | QA-done |
| Crate and module | validator, secrets/nonces.rs |
| Location | crates/validator/src/secrets/nonces.rs:36-46, 100-143, 194-219 (related: crates/validator/src/service/effect.rs:144-172, 231-235) |
| Severity | Medium / Low |
| Certainty | 42% (Critic C-VAL-B; QA may raise) |
| Assumptions involved | A9 |
| Tags | concurrency, dos, crash-consistency |

## Claim

Each group's nonce chunks are produced by a detached `std::thread`. The worker exits its loop on any sampler error, and it dies outright on any panic inside `NonceChunk::with_size` (which runs `rayon`'s global pool). Nothing observes either outcome: the `JoinHandle` is stored in a field named `_worker` and never joined, so a panicking worker produces no log line from the validator, no metric, and no state change. The `NonceStream` entry stays in the generator's map, so `NonceGenerator::start` - the only restart path, reached once per block through `Effect::ReconcileGroupSecrets` - takes its "already running" early return and can never replace the dead stream.

From that point every `Effect::NonceTree` for the group fails with `Error::Unavailable` (the worker dropped the receiving end of the request channel, so `mpsc::Sender::send` errors), which `perform_effect` degrades to `Resume::Noop`. The validator therefore stops producing nonce chunks for that group for the remaining lifetime of the process, with the only external symptom being a `warn` line per block and a `safenet_validator_effects_total{effect="nonce_tree",result="failure"}` counter that no runbook watches. Combined with F-VAL-030 the first failed top-up also leaves a permanent phantom reservation, so the validator additionally stops participating in every ceremony whose sequence lands in that chunk.

## Basis

| # | Claim | Class | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | `start` returns early for an existing entry, so a dead stream is never replaced. | E2 | crates/validator/src/secrets/nonces.rs:36-46 | excerpt 1 |
| 2 | The worker's join handle is stored in an underscore field and never joined or polled; a panic in the worker is therefore invisible to the process. | E2 | crates/validator/src/secrets/nonces.rs:100-121 | excerpt 2 |
| 3 | The stream loop breaks - ending the thread - on any sampler error. | E2 | crates/validator/src/secrets/nonces.rs:123-143 | excerpt 3 |
| 4 | Once the worker is gone the request channel is disconnected, so `next` resolves to `Unavailable` forever. | E2 | crates/validator/src/secrets/nonces.rs:194-219 | excerpt 4 |
| 5 | Reconciliation calls `retain` then `start` every block; because the group is still tracked, `retain` keeps the dead entry and `start` is the no-op from basis 1. | E2 | crates/validator/src/service/effect.rs:231-235 | excerpt 5 |
| 6 | The effect error is swallowed into `Resume::Noop` with a single `warn`. | E2 | crates/validator/src/service/effect.rs:243-255 | excerpt 6 |
| 7 | The chunk body that can panic runs on that worker thread through `rayon`'s global pool. | E2 | crates/validator/src/frost/preprocess.rs:112-131 | excerpt 7 |

### Excerpts

**`crates/validator/src/secrets/nonces.rs:36-46`**

```rust
    fn start_with_sampler(&mut self, group_id: B256, sampler: Sampler) -> Result<, Error> {
        let btree_map::Entry::Vacant(entry) = self.groups.entry(group_id) else {
            return Ok();
        };

        tracing::debug!(%group_id, "starting nonce stream for group");
        let span = tracing::debug_span!("nonce_generator", %group_id);
        let stream = NonceStream::new(sampler, span)?;
        entry.insert(stream);
        Ok()
    }
```

**`crates/validator/src/secrets/nonces.rs:100-121`**

```rust
struct NonceStream {
    _worker: thread::JoinHandle<>,
    pending: Arc<Semaphore>,
    requests: mpsc::Sender<oneshot::Sender<NonceChunk>>,
}

impl NonceStream {
    fn new(sampler: Sampler, span: tracing::Span) -> Result<Self, Error> {
        let (sender, receiver) = mpsc::channel;
        let worker = thread::Builder::new()
            .spawn(move || {
                let _guard = span.entered;
                Self::stream(sampler, receiver);
            })
            .map_err(Error::Spawn)?;

        Ok(Self {
            _worker: worker,
            pending: Arc::new(Semaphore::new(1)),
            requests: sender,
        })
    }
```

**`crates/validator/src/secrets/nonces.rs:123-143`**

```rust
    fn stream(sampler: Sampler, requests: mpsc::Receiver<oneshot::Sender<NonceChunk>>) {
        let mut rng = rand::thread_rng;
        loop {
            let started = Instant::now;
            let nonces = match sampler.nonces_chunk(&mut rng) {
                Ok(nonces) => nonces,
                Err(err) => {
                    tracing::error!(?err, "unexpected error generating nonces; aborting");
                    break;
                }
            };
            tracing::trace!(
                elapsed_ms = started.elapsed.as_millis,
                "completed nonce tree sampling effect"
            );

            if !Self::send_nonces(&requests, nonces) {
                break;
            }
        }
    }
```

**`crates/validator/src/secrets/nonces.rs:194-219`**

```rust
    fn next(&self) -> impl Future<Output = Result<Option<NonceChunk>, Error>> + 'static {
        let permit = self.pending.clone.try_acquire_owned.ok;
        let request = permit.is_some.then(|| {
            let (sender, receiver) = oneshot::channel;
            self.requests
                .send(sender)
                .map(|| receiver)
                .map_err(|_| Error::Unavailable)
        });

        // We return an `async` block instead of making this an async function,
        // which allows us to express that the future does not capture `&self`
        // and continues to live past the reference. This is useful in our
        // context as it allows callers to immediately release any lock guarding
        // the `NonceGenerator` instead of holding it for as long as it takes to
        // generate the nonce chunk.
        async move {
            let _permit = permit;
            if let Some(receiver) = request.transpose? {
                let nonces = receiver.await.map_err(|_| Error::Unavailable)?;
                Ok(Some(nonces))
            } else {
                Ok(None)
            }
        }
    }
```

**`crates/validator/src/service/effect.rs:231-235`**

```rust
                let mut generator = self.nonce_generator.lock.await;
                generator.retain(|group_id| nonces.contains_key(group_id));
                for (group_id, key_share) in nonces {
                    generator.start(group_id, key_share)?;
                }
```

**`crates/validator/src/service/effect.rs:243-255`**

```rust
impl EffectHandler<Effect, Resume> for Handler {
    async fn perform_effect(&self, effect: Effect) -> Resume {
        let kind = effect.metric_kind();
        let (resume, result) = match self.try_perform_effect(effect.clone()).await {
            Ok(resume) => (resume, EffectResult::Success),
            Err(err) => {
                tracing::warn!(?effect, %err, "failed to perform effect");
                (Resume::Noop, EffectResult::Failure)
            }
        };
        metrics::effects_total(kind, result).increment(1);
        resume
    }
```

**`crates/validator/src/frost/preprocess.rs:112-131`**

```rust
        let rngs = (0..size)
            .map({
                let seed = ChaCha12Rng::from_rng(&mut *rng)?;
                move |offset| {
                    let mut rng = seed.clone();
                    rng.set_stream(offset.checked_add(1).expect("chunk too large"));
                    Ok((offset, rng))
                }
            })
            .collect::<Result<Vec<_>, rand::Error>>?;
        let signing_share = key_share.as_key_package.signing_share;
        let (signing_nonces, leaves) = rngs
            .into_par_iter
            .map(|(offset, mut rng)| {
                let signing_nonces = round1::SigningNonces::new(signing_share, &mut rng);
                let marshaled = marshal::solidity_sign_nonces(signing_nonces.commitments);
                let leaf = nonces_leaf(offset, &marshaled);
                (signing_nonces, leaf)
            })
            .unzip::<_, _, Vec<_>, Vec<_>>;
```

## Trigger

The failure is a single event with permanent consequences; I did not find an attacker-controlled path to it, so this is a robustness finding rather than an exploit.

1. Any of: `ChaCha12Rng::from_rng(&mut *rng)` returns `Err` because the OS entropy source failed (`crates/validator/src/frost/preprocess.rs:114`, propagated through `NonceChunk::generate` to `Sampler::nonces_chunk`); or a panic unwinds out of `NonceChunk::with_size` on the worker thread - `rayon`'s `into_par_iter` propagates a worker panic to the caller, and `rayon` itself panics if the global pool cannot spawn threads under file-descriptor or memory pressure.
2. The worker thread ends (basis 3, or unwinds). Its `mpsc::Receiver` is dropped.
3. `Effect::ReconcileGroupSecrets` runs on the next `NewBlock`; the group still has a key share, so `retain` keeps the entry and `start` returns `Ok()` without doing anything (basis 1, 5).
4. The next `Effect::NonceTree` calls `next`, whose `self.requests.send(sender)` fails on the disconnected channel and maps to `Error::Unavailable` (basis 4), degraded to `Resume::Noop` (basis 6).
5. Repeats every time a top-up is attempted, for the rest of the process's life. Recovery requires an operator restart, and there is no signal telling them to perform one.

## Considered and rejected

- **"`retain` would drop the dead stream."** Checked and false: `retain` keeps every group in the reconciliation set (`crates/validator/src/secrets/nonces.rs:66-74`, called at `crates/validator/src/service/effect.rs:232`), and a group with a live key share is always in that set (`crates/validator/src/state/preprocess.rs:121-125`).
- **"The `Semaphore` permit leaks and that is the real bug."** Checked and rejected. The permit is moved into the returned future (`crates/validator/src/secrets/nonces.rs:195`, `211`) and released when that future completes or is dropped; the `cancel_then_rerequest` and `stops_ongoing_generations` tests pin exactly this behaviour (`crates/validator/src/secrets/nonces.rs:281-347`).
- **"A panic in a tokio task would abort the process."** Not applicable - this is a plain `std::thread`, not a tokio task, and the crate sets no panic hook (grep for `panic::set_hook` in `crates/validator/src` and `crates/core/src/observability` returned nothing).
- **"The existing tests cover stream death."** They do not. `group_nonce_generation` only asserts that `next` fails for a group that was never started or was explicitly retained away (`crates/validator/src/secrets/nonces.rs:246-278`); no test kills a running worker and then calls `start` again.
- **Severity.** Not High, because I could not name a trigger an attacker controls (A2 gives the adversary chain data, not the validator's OS entropy or allocator). The mechanism is `E2`; the trigger is `I`.

## Remediation options

1. Detect liveness at `start`: keep the `JoinHandle` and replace the entry when `handle.is_finished` is true, instead of returning early on any existing entry. Two lines, and it turns a permanent failure into a one-block outage.
2. Have `NonceGenerator::next` remove the entry when the send fails with a disconnected channel, so the following block's `ReconcileGroupSecrets` recreates it naturally.
3. Make the worker restart itself: wrap the body in `catch_unwind` and retry with backoff rather than `break`ing out of the loop on a sampler error.
4. Emit a dedicated metric or `error!` when a stream dies, and expose per-group nonce inventory (`available`) as a gauge so an operator can alert on a validator that has stopped preprocessing. Today nothing in `crates/validator/src/metrics.rs` covers this.

Tests to add: start a stream with a `Sampler::Custom` that returns `Err` (or panics) on its first call, then assert that a later `start` for the same group installs a working stream and that `next` succeeds.

## Trail

- Reviewer R5: drafted, self-estimate 60%. Mechanism is `E2` end to end; the death trigger itself is `I`, which is what holds the severity at Medium.

## Critic (C-VAL-B)

I read `secrets/nonces.rs` in full and `service/effect.rs:226-235` before opening the Claim.

### Independent derivation

`start_with_sampler` early-returns on an occupied `btree_map::Entry` (`nonces.rs:37-39`), and the per-block `ReconcileGroupSecrets` calls `generator.retain(...)` then `generator.start(...)` (`effect.rs:231-235`) — `retain` keeps the entry because the group is still tracked, so `start` is the no-op. `_worker` is never joined (`nonces.rs:101`), and `NonceStream::stream` `break`s out of its loop on any sampler error (`nonces.rs:127-133`). After either exit the `mpsc::Receiver` is dropped, so `self.requests.send(sender)` fails and `next` resolves to `Error::Unavailable` (`nonces.rs:198-202`), which `perform_effect` turns into `Resume::Noop`. The supervision gap is exactly as claimed and I reached it independently.

### Per-claim verdicts

All seven basis rows **Supported**; every quote matches this checkout. No `H` claims.

### Where I part company: the trigger is not established

The mechanism is a _consequence_ of a worker dying; the finding does not show that a worker can die. I looked for a path and could not find one either:

- The error branch requires `sampler.nonces_chunk` to return `Err`. That is `rand::Error` from `ChaCha12Rng::from_rng(&mut *rng)?` over `ThreadRng` (`frost/preprocess.rs:112-114`) — an OS entropy failure, which is not something chain input can cause.
- The panic branch requires a panic inside `NonceChunk::with_size`. The only `expect` there is `offset.checked_add(1).expect("chunk too large")` (`frost/preprocess.rs:117`) with `offset < 1024`, and `MerkleTree::build`/`proof` are total (`merkle.rs:21-22`, `:55` all use `get(..).unwrap_or`). Everything else is `round1::SigningNonces::new`, whose panic behaviour is `frost-core` internals and therefore class `I` under A6 (sources not on disk, `state/baseline.md` §1).
- The `send_nonces` exits are the _designed_ shutdown when `retain` drops a group's stream, not a fault.

No untrusted input reaches any of these. Per the brief's own calibration rule ("a panic that no untrusted input can reach is Low or Informational however alarming it looks"), that caps this.

### Finding verdict

**Plausible — 42%.** The mechanism (unsupervised worker, unreplaceable dead stream, failure degraded to `Resume::Noop`) is `E2` and fully verified. The trigger is unproven: no reachable death path exists in this checkout, and the one that cannot be excluded is a dependency internal, class `I`.

**Severity: Medium → Low.** Not Medium: nothing an attacker or a peer does reaches the failing code, and the consequence only materialises after a fault that is itself unreachable from chain input. Not Informational: if the worker ever does die the validator stops producing nonce chunks for the rest of the process lifetime with no metric that distinguishes it (the only signal is `effects_total{result="failure"}`, see F-VAL-061), and the fix — replace a dead entry in `start` rather than early-returning, and check `_worker.is_finished` — is small and self-evidently correct.

**QA note.** This is the cheapest of the sixteen to settle with a toolchain: `#[cfg(test)]` a `Sampler::Custom` that panics, then assert that a subsequent `start` replaces the entry and that `next` succeeds. `Sampler::Custom` already exists for exactly this kind of test (`nonces.rs:84-86`).

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **42%**; severity Medium / Low unchanged. No PoC directory: the finding is not in my assigned PoC set, and C-VAL-B is right that this is the cheapest of the cluster to settle once a toolchain exists — the hook already exists in the crate.

### What would be run, and what it would show

`Sampler::Custom` exists for exactly this (`crates/validator/src/secrets/nonces.rs:84-88`), and `start_with_sampler` is reachable from `crate::secrets::nonces`'s own test module. The test is:

```rust
// crates/validator/src/secrets/nonces.rs, tests module
#[tokio::test]
async fn a_dead_stream_is_replaced_on_the_next_start {
    let mut generator = NonceGenerator::new();
    generator.start_with_sampler(GROUP, Sampler::Custom(Box::new(|_| Err(rand::Error::new(..))))).unwrap();
    assert!(generator.next(GROUP).await.is_err);          // the worker has now `break`ed out
    generator.start(GROUP, Arc::new(KeyShare::dummy())).unwrap();
    assert!(generator.next(GROUP).await.unwrap.is_some); // FAILS TODAY
}
```

The second `start` returns `Ok()` without doing anything, because `btree_map::Entry::Vacant` does not match an occupied entry (`nonces.rs:37-40`), so the dead stream is never replaced. Passing that assertion is the acceptance test for remediation option 1.

**A pass today would refute the finding**; a failure confirms the mechanism at `E1` while leaving the trigger where C-VAL-B put it, because nothing in this checkout reaches the sampler error path from chain input. The certainty should therefore stay at 42 even after the test runs — the test settles the _mechanism_, which is already `E2`, not the trigger, which is the part in doubt. Worth saying plainly so the run is not over-read.

### Remediation check

**Option 1 (check `is_finished` and replace the entry in `start`) is sound and is the fix.** Two lines, and it turns a permanent failure into a one-block outage because `ReconcileGroupSecrets` calls `start` for every retained group on every block (`service/effect.rs:233-235`). One caution: `JoinHandle::is_finished` is `false` for a thread that is alive but wedged, so this catches death and not hangs; that is the right scope, but do not let it be described as "detects a stuck worker".

**Option 2 (remove the entry when the send fails with a disconnected channel) is sound and is strictly better than option 1 in one respect and worse in another.** Better: it detects the death at the moment it matters, inside `next`, rather than one block later. Worse: `NonceGenerator::next` takes `&self` (`nonces.rs:54-62`) and returns a `'static` future precisely so the mutex is dropped before the await (`service/effect.rs:155-158`) — removing the entry needs `&mut self` and would either reintroduce holding the lock across the await or require interior mutability. Take option 1 first; option 2 needs a design note, not just two lines.

**Option 3 (`catch_unwind` plus retry with backoff) is sound for panics and not for the `Err`-returning path**, and the finding conflates them. The `break` at `nonces.rs:132-136` is reached by a sampler `Err`, not a panic; a `catch_unwind` would not see it. Retrying with backoff instead of `break`ing is the change that matters, and it is one line. Also note `catch_unwind` requires the closure to be `UnwindSafe`, which the `Sampler` and channel captures are not without an `AssertUnwindSafe` — worth flagging so it is not scoped as trivial.

**Option 4 (a metric or `error!` on stream death, plus an `available` gauge) is the one that should ship regardless of the others.** Today a validator whose worker has died is indistinguishable from a healthy one at every layer: `next` returns `Err(Unavailable)`, `perform_effect` swallows it into `Resume::Noop` (F-VAL-061), and the only counter is `effects_total{result="failure"}` whose `Success` label explicitly covers expected no-ops (`metrics.rs:91-92`). The `available` gauge is shared with F-VAL-030 option 4 and F-VAL-061 option 5; build it once.
