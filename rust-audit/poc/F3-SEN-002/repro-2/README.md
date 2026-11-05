# F3-SEN-002 second reproduction (QA3-2B)

Real `sentinel` binaries against a real Consensus + SentinelOracle on local Anvil, Aegis oracle parameters (fee 0.40, bond 2000x = 800, slash 1x, commit 6, reveal 6). Fee token is MyToken so the Direct-mode `approveToken` (55,000 gas) succeeds, isolating the state-machine defect from F3-SEN-001 (which is about USDC.e's higher approve cost).

Route: the adversarial "never finalizes" case (case 2), distinct from the obvious honest reveal-order race. A silent registered sentinel M commits while the two honest sentinels A and B still wait for their engine verdicts (their engines are held just long enough to span M's commit, the finding's "before the honest engines answer" trigger). M never reveals.

## Command

```
source .../env/qa-2b.sh
python3 -I sen002.py <sentinel-binary> <label>
```

## Result: reproduced

Pristine `sentinel-361aaf3`:

```
both honest sentinels logged 'ignoring unexpected commitment' for M's commit
FINAL onchain request (>= revealDeadline+20): state=PENDING committed=3 revealed=2 approve=2 deny=0
commitment[A] vote=APPROVED claimed=False bond=800000000
commitment[B] vote=APPROVED claimed=False bond=800000000
commitment[M] vote=PENDING  claimed=False bond=800000000
A: finalize block 24 status=0 replay=FinalizeTooEarly()
B: finalize block 24 status=0 replay=FinalizeTooEarly()
verdict: stuck_pending=True  A_sent_reverted_finalize=True  B_sent_reverted_finalize=True  no_oracle_outcome_event=True
```

Each honest sentinel undercounts (committed_count 2 vs onchain 3), early-finalizes at the undercount, sends a `finalize` that reverts `FinalizeTooEarly`, and parks in WaitingForOutcome. Nobody finalizes after the reveal deadline: the request stays PENDING and all three 800 bonds stay locked.

## Bite check: sound

Remediation option 1 (remove the early finalize from `handle_revealed`), rebuilt sentinel:

```
FINAL request: state=RESOLVED_APPROVED committed=3 revealed=2
A: finalize block 28 status=1 (revealDeadline+1)   B: finalize block 28 status=0 (redundant, A won)
commitment[A] claimed=True   commitment[B] claimed=True
verdict: stuck_pending=False  no_oracle_outcome_event=False
```

With the early finalize gone, the reveal-deadline branch finalizes once (succeeds onchain) and both honest sentinels claim. Remediation sound.
