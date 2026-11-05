# F3-SEN-003 second reproduction (QA3-2B)

Real `sentinel` binaries, Aegis oracle parameters, MyToken fee token (Direct mode works).

Route: outage via SIGSTOP across the reveal window, NO process restart (so no snapshot rollback or warp, isolating this from F3-SEN-004). The committed sentinel B is paused for its whole reveal window while A reveals and finalizes; on resume B's reveal is too late.

## Command

```
source .../env/qa-2b.sh
python3 -I sen003.py <sentinel-binary> <label>
```

## Result: reproduced

Pristine `sentinel-361aaf3`:

```
both committed; B paused (SIGSTOP) for the whole reveal window
after A finalizes: state=RESOLVED_APPROVED committed=2 revealed=1
B resumed (SIGCONT) past the reveal deadline
FINAL request: state=RESOLVED_APPROVED
commitment[A] vote=APPROVED claimed=True
commitment[B] vote=PENDING   claimed=False bond=800000000
B balance = 79200000000   (one whole 800 bond locked)
B: reveal block 29 status=0 replay=RequestNotPending()   (late, reverts)
B log (trace): "ignoring oracle result for an untracked request"
verdict: B_vote_still_PENDING=True  B_never_claimed=True  B_sent_no_claim=True  B_no_reveal_missed_metric=True
```

B drops the request at NewBlock(revealDeadline+1) before the OracleResult log of that block, so the `RevealMissed` recovery in `handle_oracle_result` never runs (confirmed by the untracked-result trace). B never claims its 799.60 remainder: against the 0.40 the protocol slashes, the state machine strands the whole bond. (B's late reveal reverts; it is not a successful reveal.)

## Bite check: sound (claim recovered)

Remediation option 1 (in `finalize()`, keep a committed-but-unrevealed request in WaitingForOutcome instead of dropping it):

```
commitment[B] vote=PENDING claimed=True
B: claim block 29 status=1
B balance = 79999600000   (only the 0.40 governed slash lost; 799.60 recovered)
```

The 799.60 previously stranded is recovered. Remediation sound. Note: recovery routes through WaitingForOutcome, whose `self_revealed()` is `true`, so the metric is `Unanimous` rather than `RevealMissed`; a precise `RevealMissed` label would need the state to carry self_revealed=false.
