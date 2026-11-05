# F3-VAL-030 second reproduction (QA3-2A)

Route: drive the real signing state machine (`sign.rs`) at Aegis parameters and show the validator acts on an approving OracleResult in the block it sees it, broadcasting a signShareWithCallback (callback = attestTransaction) within about two blocks, with no reorg-safety wait. This is the finding's root cause (basis 1-4, Critic row 1), a different route from the primary QA's full two-validator Anvil reorg.

Not executed here (cited in the finding, primary QA's route): the reorg rollback (driver snapshot, basis 5), the queue rebroadcast of the allocated share (basis 6), and the contracts accepting the attestation with no oracle gate (basis 7-9; the guard side is settled design). This PoC isolates the validator action that makes those downstream facts harmful.

Key material is real: a full 3-of-5 FROST DKG produces the key shares, the nonce commitments and this validator's signature share. The only setup is placing the validator in WaitingForOracle, the normal pre-result state; no mocked signatures.

Worktree: wt/rv-core-b. Env: env/rv-core-b.sh.

## Reproduce (defect present)

```sh
source <run folder>/env/rv-core-b.sh
cd "$WT"
printf '\n' >> crates/validator/src/state/sign.rs
cat rust-audit/poc/F3-VAL-030/repro-2/qa2a_val030_no_finality_wait.rs >> crates/validator/src/state/sign.rs
cargo test --offline -p validator --bins qa2a_val030 -- --nocapture --test-threads=1
git checkout -- crates/validator/src/state/sign.rs
```

Pass: block 1000 approval -> GenerateNonces now, signing deadline 1006 (no +max_reorg_depth); block 1001 -> UseNonces; SignShare with attestTransaction callback, expires_at 1007. See `baseline.out`.

## Bite (remediation option 1)

Patch the approving arm of `handle_oracle_result` to hold in WaitingForOracle instead of emitting GenerateNonces (a full fix re-emits it on the first NewBlock at or past result_block + max_reorg_depth). With the patch the validator commits to nothing within the reorg window, so the "GenerateNonces emitted" assertion fails. See `with-fix.out`. Remediation is sound (it removes the within-window share); cost is about 25 s extra latency per attestation on Gnosis, covered by oracle_timeout 24.
