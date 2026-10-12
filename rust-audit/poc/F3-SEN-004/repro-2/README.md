# F3-SEN-004 second reproduction (QA3-2B)

Real `sentinel` binaries, Aegis oracle parameters, MyToken fee token (Direct mode works).

Route: process restart with ~7 blocks of downtime, the real warp-replay path (the path F3-SEN-003 avoids). Sentinel A commits, is cleanly stopped, and restarts; the warp replays the proposal, NewRequest and A's OWN Committed in one update before the engine check resumes. Sentinel B stays up and reveals, so A's dropped request strands A's bond.

## Command

```
source .../env/qa-2b.sh
python3 -I sen004.py <sentinel-binary> <label>          # reproduction and companion-fix bite
python3 -I sen004_bite.py <sentinel-binary> <label>     # option-1-only timing probe
```

## Result: reproduced

Pristine `sentinel-361aaf3`:

```
A stopped (clean), exit 0; downtime ~7 blocks
A restarted at head 20
initializing block watcher {latest: 20, safe: 15, resume: Some(BlockStatus { latest: 12, safe: 7 })}
WARN ignoring unexpected commitment {state: waiting_for_engine_check}   (x2: A's and B's commits in the warp page)
FINAL request: state=RESOLVED_APPROVED committed=2 revealed=1
commitment[A] vote=PENDING claimed=False bond=800000000
A balance = 79200000000   (whole 800 bond stranded)
verdict: A_vote_still_PENDING=True A_never_claimed=True A_no_successful_reveal=True A_own_commit_ignored_on_replay=True
```

A's own Committed is ignored during the warp (handle_committed sees WaitingForEngineCheck), so self_committed is lost, A drops the request at commitDeadline+1 with no reveal, the request resolves on B alone, A is slashed and its 799.60 remainder stranded.

## Bite check: option 1 alone insufficient; option 1 + companion is sound

`sen004_bite.py` (chain paused at commitDeadline) shows remediation option 1 alone (track self_committed through WaitingForEngineCheck, carry into commit_vote) does NOT restore the reveal: the state machine's optimistic pending-block transition applies NewBlock(commitDeadline+1) and `handle_block_advance` drops the WaitingForEngineCheck entry before the (verdict pruned on restart, so fresh) engine check resumes ("ignoring stale engine check result"). The named defect is fixed (the "ignoring unexpected commitment" is gone) but the request is still dropped via the WaitingForEngineCheck arm.

Adding the companion change (retain a self_committed WaitingForEngineCheck entry until reveal_deadline), `sen004.py` with the combined fix:

```
reusing recorded engine verdict {Approved}
A: reveal block 21 status=1   A: claim block 22 status=1
commitment[A] vote=APPROVED claimed=True
A balance = 80000200000   (bond returned in full + 0.20 fee share)
verdict: A_no_successful_reveal=False  A_never_claimed=False
```

A reveals and claims; the combined fix is sound. Recorded QA result: the finding's remediation option 1 is necessary but not sufficient on a real restart whose head is already past the commit deadline.
