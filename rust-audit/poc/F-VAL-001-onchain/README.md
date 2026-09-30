# F-VAL-001 Phase 8 onchain reproduction

Runs the F-VAL-001 attack sequence against the REAL FROSTCoordinator + FROSTParticipantMap bytecode (no mocks). Kept out of the tracked contracts/test tree; run from the scratchpad via an out-of-tree test dir:

    SC=<scratchpad>
    cp FVal001Onchain.t.sol "$SC/ftest/"
    FOUNDRY_TEST="$SC/ftest" forge test --root contracts --match-path '*FVal001Onchain*' -vv

Asserts, against real contract bytecode: (1) an impostor registers a keyGenCommit whose `q` is a verbatim copy of the victim's `q`; (2) one plaintiff files n-1 complaints and the group is never COMPROMISED; (3) honest accused answer with keyGenComplaintResponse; (4) the impostor calls keyGenConfirm and the group FINALIZES with the impostor holding a participant key. 5/5 passes on fresh seeds.

## Post-merge re-run (RV-VAL)

Re-run unchanged against merge commit `a7f3915` (origin/main merged in, carrying the Certora FROST audit fixes I-01..I-09). `forge clean` + full recompile of the merged `contracts/src`, then 5 seeds + one clean-build seed: **6/6 PASS**, gas ~2.087M (identical to the pre-merge run). Output: `RESULT-postmerge-rv-val.txt`.

No edit to the harness was required, which is itself evidence: every contract symbol and signature the attack touches is unchanged.
