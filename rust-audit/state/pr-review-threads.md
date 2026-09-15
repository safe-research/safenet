# PR #905 — reviewer threads and dispositions

Inline review comments on this PR from the team, mapped to the finding each one addresses (by the report or finding line the comment was left on, at commit `57044af`). GitHub exposes a thread's _resolved_ flag only through the authenticated GraphQL API, so this records replies and reactions, not resolution.

**Summary:** 13 team comments on 10 findings. Copilot's two earlier threads (`--lib` → `--bins`; Prettier) were fixed and acknowledged. **None of the 13 team threads has a reply yet.** Two assumptions the team proposed in these threads are now adopted as **A16** (genesis not recoverable) and **A17** (databases touched only by the services), and the engine was placed out of scope as asked.

| Finding | Reviewer | Where | Comment | Disposition | Audit note / suggested reply |
| --- | --- | --- | --- | --- | --- |
| [`F-ENG-030`](../findings/F-ENG-030.md) | @rmeissner | `report/REPORT.md:123` | I would ignore all engine changes for now | **Scope** | Engine placed out of scope; all engine artefacts removed from this PR. |
| [`F-VAL-005`](../findings/F-VAL-005.md) | @rmeissner | `report/REPORT.md:131` | this should be fixed with the prune refactor (cc @nlordell ) | **Fix claimed (prune refactor)** | Pruning stack #906–#913 is now on `main`. Run-1 assessment: **partially fixed** — the stored-block ordering closes the same-height replay (live: epoch-1 commitments identical across a reorg), but a group absent for more than `max_reorg_depth` blocks is still deleted (`F-VAL-068` D2). Run 2 re-checks on the merged code. |
| [`F-SEN-002`](../findings/F-SEN-002.md) | @rmeissner | `report/REPORT.md:134` | This is something we should fix I will take this on. Assignee: @rmeissner | **Assigned @rmeissner** | Note before fixing: #914 merged **without** #915 makes this worse — a valid peer commit in the deadline block is discarded (`F-SEN-016` D1). |
| [`F-SEN-015`](../findings/F-SEN-015.md) | @rmeissner | `report/REPORT.md:137` | This should also be adjusted. I will taker this on Assignee: @rmeissner | **Assigned @rmeissner** |  |
| [`F-VAL-004`](../findings/F-VAL-004.md) | @rmeissner | `report/REPORT.md:142` | I would say this is accepted, as genesis is an exception. If this affects other flows this could change. | **Accepted (genesis exception)** | Adopted as assumption **A16**; genesis-only liveness findings become Informational/`known`. |
| [`F-VAL-032`](../findings/F-VAL-032.md) | @rmeissner | `report/REPORT.md:143` | I would say this is accepted | **Accepted** |  |
| [`F-CORE-067`](../findings/F-CORE-067.md) | @rmeissner | `report/REPORT.md:148` | they will have different nonces and one of them will revert, right? | **Question** | **Yes.** Live evidence (Phase 8): the duplicated `approve`+`commit` took nonces 2 and 3, and the duplicate `commit` reverted with `AlreadyCommitted()` (`0xbfec5558`). Impact is the gas of a reverting transaction plus an invisible failure — the queue infers success from the account nonce (`F-CORE-063`), so the revert never surfaces (`F-SEN-006`). Not a double state change; Medium stands, and run 2 will re-rate. |
| [`F-SEN-005`](../findings/F-SEN-005.md) | @rmeissner | `report/REPORT.md:151` | This should be an easy fix. Assignee: @rmeissner | **Assigned @rmeissner (easy fix)** | Note: #914 does not close it — `event.deadline` is put on the wire but never read, and both waiting states still never expire (`F-SEN-016`). |
| [`F-SEN-003`](../findings/F-SEN-003.md) | @rmeissner | `report/REPORT.md:164` | @nlordell this is an interesting one. Why was it decided to not trigger any `NewBlock` when warping? | **Question to @nlordell** | Design question, not an audit claim. On `main` the frozen-bond loss is closed by `handle_dispute_triggered`'s recovery; the replayed reveals are still discarded. |
| [`F-VAL-033`](../findings/F-VAL-033.md) | @nlordell | `report/REPORT.md:46` | I think we should have added that genesis lockups are not recoverable by design. I still need to understand the nonce reuse issue it claims though. | **Objection + proposed assumption** | Adopted as **A17** (only the services touch their databases): `F-VAL-033` is **out of scope**. The objection to remediation option 1 is correct — a restore can drop the consumed-nonce row as easily as the chunk row. Its live status was already reduced (Critical → High): the validator self-halts before any nonce is reused. |
| [`F-CORE-033`](../findings/F-CORE-033.md) | @nlordell | `findings/F-CORE-033.md:19` | Interesting, as I would expect the transaction deadline to prevent this fan-out. | **Question** | Open: whether the request/transaction deadline bounds `EngineCheck` effects spawned from a warp page. Routed to run 2 for verification. |
| [`F-VAL-033`](../findings/F-VAL-033.md) | @nlordell | `findings/F-VAL-033.md:210` | Umm.... the database can be restored with this row missing too???? | **Objection + proposed assumption** | Adopted as **A17** (only the services touch their databases): `F-VAL-033` is **out of scope**. The objection to remediation option 1 is correct — a restore can drop the consumed-nonce row as easily as the chunk row. Its live status was already reduced (Critical → High): the validator self-halts before any nonce is reused. |
| [`F-VAL-033`](../findings/F-VAL-033.md) | @nlordell | `findings/F-VAL-033.md:1` | For this one, I think an added assumption would be that the database is _only_ modified by the sentinel and validator processes (otherwise, there is all kinds o | **Objection + proposed assumption** | Adopted as **A17** (only the services touch their databases): `F-VAL-033` is **out of scope**. The objection to remediation option 1 is correct — a restore can drop the consumed-nonce row as easily as the chunk row. Its live status was already reduced (Critical → High): the validator self-halts before any nonce is reused. |

## Open questions the team asked

1. **`F-CORE-067`** (@rmeissner) — _different nonces, one reverts?_ Yes; see the note above. Suggested reply drafted there.
2. **`F-CORE-033`** (@nlordell) — _the transaction deadline should prevent the fan-out._ Not yet answered; run 2 verifies whether the deadline bounds effects spawned from a warp page.
3. **`F-SEN-003`** (@rmeissner → @nlordell) — _why no `NewBlock` on warp?_ A design question for the team.
4. **`F-VAL-033`** (@nlordell) — _I still need to understand the nonce reuse issue._ Under A17 the finding is out of scope; the mechanism (a restore un-burns a nonce) is real but requires an operator restore, which A17 excludes.

## Status changes driven by these threads

- Accepted: `F-VAL-004` (via A16), `F-VAL-032`.
- Out of scope: `F-VAL-033` (via A17); all `F-ENG-*` (engine removed).
- Assigned: `F-SEN-002`, `F-SEN-015`, `F-SEN-005` → @rmeissner.
- Fix claimed via merged work: `F-VAL-005` (pruning stack) — partially, per run-1's assessment; run 2 re-verifies.
