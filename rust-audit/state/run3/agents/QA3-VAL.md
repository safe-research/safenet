# QA3-VAL coverage log (2026-10-08)

Primary QA, validator crate. Worktree `wt/rv-val`, Anvil ports 31400 to 31409. All PoC hunks kept in the worktree and reverted; final `git status --short` clean.

## Findings handled

| Finding | Result | PoC | Certainty |
| --- | --- | --- | --- |
| F3-VAL-030 | Reproduced (2 of 2), re-inclusion + guard path | `poc/F3-VAL-030/` (two real validators + Anvil `anvil_reorg`) | 96 |
| F3-XC-100 (canonical; F3-VAL-001 dup) | Reproduced (state-machine glue) | `poc/F3-XC-100/` (transition test, Aegis 5/3) | 95 |
| F3-VAL-062 | Reproduced (share drop + deadline drift) | `poc/F3-VAL-062/` (transition test, live vs warp) | 92 |
| F3-VAL-031 | Reproduced impact rule; reorg trigger traced | `poc/F3-VAL-031/` (transition test) | 60 |
| F3-VAL-032 | Reproduced claim 1 (fresh pair after collection) | `poc/F3-VAL-032/` (store unit test) | 90 |
| F3-VAL-061 | Reproduced genesis half | `poc/F3-VAL-061/` (transition test) | 90 |

## Commands

- Build: `cargo build -p validator --bins`; `forge build --root contracts` (warm cache).
- F3-VAL-030: `bash poc/F3-VAL-030/reorg_oracle_poc.sh` (reproduce + guard path), `bash poc/F3-VAL-030/reorg_oracle_bite.sh` (fixed depth-4 reorg, run unpatched then with remediation 1).
- Transition / unit PoCs: `cargo test -p validator --bins <test_name> -- --nocapture` for `identity_share_excludes_victim_and_skips_epoch`, `warp_order_drops_peer_share_and_delays_deadline`, `split_shares_drop_the_ceremony` / `threshold_selection_restarts_instead_of_dropping`, `collected_signature_id_is_given_a_fresh_pair`, `divergent_genesis_set_silently_ignores_the_deployment_key_gen`.

## Method notes

- No validator-crate state-machine (`Transition`) test harness exists, so each PoC adds a temporary `#[cfg(test)]` module in the real source file and drives `apply_transition`, then reverts. Shared DKG attack / honest vectors live in a temporary `frost::identity_attack` helper (crafts the cancelling `c[1]` and honest shares via the real `keygen` API).
- F3-VAL-030 ran the real binaries end to end (SimpleOracle for manual approve/reject); `anvil_reorg` reverts contract storage, and an injected replacement tx needs the post-reorg nonce, so the PoC calls `reject` after the reorg rather than injecting it.

## Not attempted / partial

- F3-VAL-031 reorg origin (early nonce burn on a non-final block, split RPC views) not driven; only the drop-vs-restart rule executed.
- F3-VAL-061 rollover half and `blocks_per_epoch`/timeout divergences, and the real-binary-against-Anvil run, not executed; genesis-id mismatch executed.
- F3-VAL-060 is outside my assignment.

## Time

About one working session (build warm). Four background jobs for F3-VAL-030 (reproduce x2, bite unpatched, bite patched); all exited and no Anvil left on my ports.
