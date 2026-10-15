# QA3-2B coverage log (second reproducer, discovery track)

Commit `361aaf3`. Worktree `wt/qa-2b`, env `env/qa-2b.sh`, Anvil ports 31700 to 31709. Fresh worktree warmed with `run-bg r3-qa-2b-warm warm-chain qa-2b` (cargo build + test --no-run + forge build, all rc 0, about 100 s). Pristine `sentinel` and `validator` binaries saved to `scratch/QA3-2B/bin/` before any code hunk. Independence held: read each finding only to and including its Critic section before writing my own reproduction, never opened the primary QA's `poc/F3-<id>/` folders (outside `repro-2`) or QA sections while reproducing.

Harness: `scratch/QA3-2B/h.py` (stdlib only) drives one Anvil (chain 100, so Gnosis 5 s timings), deploys the real Consensus (TestConsensus) + SentinelOracle with Aegis parameters, runs the real `sentinel` binaries with file-backed SQLite and a controllable engine stub and a mining thread (automine off, one block per 5 s wall-clock so transaction order per block is observable), plus a loopback JSON-RPC proxy whose per-method policy a test controls. Fee token is MyToken, whose Direct-mode approve (about 47k gas) fits the 55,000 limit, so the sentinel state-machine findings reproduce in Direct mode independently of F3-SEN-001 (which is about USDC.e's higher approve cost).

| Finding | Result | Route | PoC |
| --- | --- | --- | --- |
| F3-CORE-001 | Reproduced, bite sound | real sentinel + rate-limiting proxy: blackout forces per-event mode, concurrency cap < 11 stalls it; /health stays OK, ~90 getLogs/s | `poc/F3-CORE-001/repro-2/` |
| F3-CORE-002 | Reproduced, bite sound | real sentinel, use_client_filtering, proxy empties the proposal block's logs; block committed empty, event lost | `poc/F3-CORE-002/repro-2/` |
| F3-CORE-030 | Reproduced, bite sound | real sentinel + two Anvils (fork behind oldest snapshot): BadUpdate, exit 0 | `poc/F3-CORE-030/repro-2/` |
| F3-SEN-002 | Reproduced, bite sound | real sentinels, adversarial silent-M "never finalizes" case; request stuck PENDING, reverted finalizes | `poc/F3-SEN-002/repro-2/` |
| F3-SEN-003 | Reproduced, bite sound | real sentinels, SIGSTOP across the reveal window (outage, no restart); 799.60 stranded, recovery unreachable | `poc/F3-SEN-003/repro-2/` |
| F3-SEN-004 | Reproduced, bite nuanced | real sentinels, restart + warp replay ignores own commit; option 1 alone insufficient, needs a companion change | `poc/F3-SEN-004/repro-2/` |
| F3-XC-160 | Reproduced, bite sound | watcher-level test on the real combined Watcher: null revalidation reports a false uncle | `poc/F3-XC-160/repro-2/` |

Commands (each prefixed by `source env/qa-2b.sh`, run from `scratch/QA3-2B`):

- `python3 -I core001.py <bin> <label>`; `python3 -I core002.py <bin> <label> [suppress_s]`; `python3 -I core030.py <bin> <label>`.
- `python3 -I sen002.py <bin> <label>`; `python3 -I sen003.py <bin> <label>`; `python3 -I sen004.py <bin> <label>`; `python3 -I sen004_bite.py <bin> <label>`.
- `cargo test --package safenet-core --lib index::tests::qa3_2b_xc160` (XC-160 watcher hunk, reverted after).

Bite fixes applied in the worktree and reverted (clean `git status --short`): blocks.rs initialize wait (CORE-030); events.rs sequential per-event (CORE-001) and may_contain_log fallback (CORE-002); service.rs remove early finalize (SEN-002), keep-in-WaitingForOutcome (SEN-003), and state.rs/service.rs count-through-WaitingForEngineCheck plus the handle_block_advance companion (SEN-004); blocks.rs revalidate None-is-not-uncle (XC-160). Each rebuilt only `cargo build -p sentinel` (or the XC-160 test), so the finding's own test module (old shape) never had to compile.

Not attempted: none of the seven. No live RPC used; all chain state is local Anvil. Ad-hoc Anvil on ports 31700 to 31701 only, killed at the end of each run (`pgrep -a anvil` clean). Builds used the shared build slots via the `cargo` wrapper.
