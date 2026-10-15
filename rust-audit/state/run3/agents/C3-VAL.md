# C3-VAL coverage log: Critic, validator findings (run 3, discovery track)

| Field | Value |
| --- | --- |
| Agent | C3-VAL (auditor department), Opus 5.5 |
| Commit | `361aaf3` (main checkout; `crates/`, `contracts/src`, `docs/`, `scripts/` unmodified, checked with `git status --short`) |
| Date | Started 2026-10-08, finished after midnight on 2026-10-09; the Critic section headers use the run date from the brief |
| Assignment | `F3-VAL-001`, `F3-XC-100`, `F3-VAL-030`, `031`, `032`, `F3-VAL-061`, `062`; rejected lists of R4, R5, R6, L-crypto; promotions `F3-VAL-090` to `099` |
| Extra task (head, mid-run) | Settle R4 O2 against R6 Q29, R4 Q30 against R6 O6, and O-COV-1 (`blocks_per_epoch`) |
| Result | 6 Confirmed, 1 Plausible, 0 Refuted; 1 severity change (`F3-VAL-030` Critical to High); 1 duplicate settled; 0 promotions |

## Independence

- Read: the shared Critic brief, `PROMPT.md`, `state/run3/{STATE,settled,baseline,map}.md`, every assigned `F3-*` file, `F3-CORE-032.md` (overlap check for `F3-VAL-062`), the coverage logs of R4, R5, R6 and L-crypto, `state/run3/logs/{L-crypto-identity-share,R4-cancelled-shares,R6-keygen-warp-order}.txt`, the maintainers' project notes and the team's decisions.
- Read outside `rust-audit/`, to settle known and decided status only: the lines of the maintainers' review notes that mention 1025 and 1033 (team PR-review notes, not audit findings), and the Safenet Charter Article I (`safenet-charter`, `6ef1aa9`).
- Not opened: `findings/F-*`, `F2-*`, `report/`, `poc/`, `codebase-map.md`, `analysis/`, `state/run3/revalidation/`, the `rv-*` logs. Names only, never contents: the session's initial git status and one `git status --short` listed run-1 and run-2 finding and PoC folder names; `STATE.md` (an allowed file) summarises re-validation outcomes, which I did not use.

## Findings handled

| ID | Verdict | Severity (reviewer / critic) | Certainty | Duplicate | Notes |
| --- | --- | --- | --- | --- | --- |
| `F3-VAL-030` | Confirmed | Critical / High | 80 | canonical | Impact class Critical; High for the natural, unsteerable trigger. The guard needs no re-inclusion (shares are public), so only remediation 1 closes it. Minimum depth 3 for every flip that drops a deny vote; depth 2 for the benign flip to no votes. L-crypto O6 found the same mechanism. |
| `F3-XC-100` | Confirmed | High / High | 88 | canonical | Same defect as the team's open item from the review of #1025 (Medium). Contract-only fix leaves the victim unable to sign (`getKey`). |
| `F3-VAL-001` | Confirmed | High / High | 85 | duplicate of `F3-XC-100` | Adds the two-victim variant at threshold 3; trigger step 3 needs the withheld share. |
| `F3-VAL-031` | Plausible | Low / Low | 50 | canonical | L-crypto O5 found the same split. Docs claim (`overview.md:83`) contradicted. |
| `F3-VAL-032` | Confirmed | Informational / Informational | 80 | canonical | Claim 2 is known (`settled.md` section 2; the review of #1033); claim 1 is new. |
| `F3-VAL-061` | Confirmed | Low / Low | 75 | canonical | O-COV-1 (`blocks_per_epoch`) added as a trigger in my Critic section. |
| `F3-VAL-062` | Confirmed | Medium / Medium | 80 | canonical | Related to `F3-CORE-032`, not a duplicate. |

No refutation, so no rebuttal round is needed (reviewer self-estimates of 70 or more: `F3-VAL-001` 70, `F3-XC-100` 80, `F3-VAL-062` 75, all confirmed).

## Citations re-opened at `361aaf3`

- `F3-VAL-030`: `state/sign.rs:21-112, 152-223, 233-332, 339-384, 492-693, 757-800`; `state/mod.rs:396-472`; `state/transactions.rs:16-77`; `service/action.rs:248-292`; `crates/core/src/tx/mod.rs:166-330`; `tx/storage.rs:165-180, 314-410`; `index/blocks.rs:125-140, 296-346, 431-530`; `driver.rs:230-300`; `state/mod.rs:170-290` (core); `Consensus.sol` (all); `FROSTCoordinator.sol:490-712`; `FROSTSignatureShares.sol:60-140`; `FROST.sol:49-53, 192-203`; `SafenetGuard.sol:160-265`; `SentinelOracle.sol` (all); `SentinelOracleRequests.sol` (all); `crates/sentinel/src/service.rs:200-260, 396-480`; `docs/overview.md` (all); `docs/transaction-lifecycle.md` (all); `git show fe9e84c:crates/validator/src/state/sign.rs` lines 185-200.
- `F3-XC-100`, `F3-VAL-001`: `frost/keygen.rs` (all); `frost/marshal.rs` (all); `frost/serialization.rs` (all); `frost/mod.rs:280-325`; `state/keygen.rs:30-150, 150-430, 645-850, 920-1240, 1318-1430`; `consensus/group.rs:140-300`; `FROSTParticipantMap.sol:120-256`; `FROSTCoordinator.sol:355-440`; `Secp256k1.sol:151-196`; `git log -1 7fc1e08`.
- `F3-VAL-031`: `state/sign.rs` (all); `secrets/store.rs:1-60, 205-300, 355-385`; `service/effect.rs:20-170`; core `state/mod.rs:170-290`; `SentinelOracleRequests.sol:391`.
- `F3-VAL-032`: `secrets/store.rs:18-22, 38-41, 226-233, 258-266, 355-385`; `service/effect.rs:32-35`; `frost/sign.rs:1-130`; `frost-core-3.0.0/src/round2.rs:125-150` (registry).
- `F3-VAL-061`: `main.rs:30-99`; `service/mod.rs:36-80`; `config.rs:40-110`; `bindings.rs:160-168`; `consensus/group.rs:140-300`; `state/keygen.rs:30-64, 1130-1240`; `FROSTCoordinator.sol:398-408`; `docs/configuration.md:7`.
- `F3-VAL-062`: `state/keygen.rs:64-150, 150-300, 296-430, 1000-1130, 1240-1256`; core `index/blocks.rs:296-346`, `state/mod.rs:48-60, 170-290`, `driver.rs:230-300`; `state/run3/logs/R6-keygen-warp-order.txt`.

Every Basis row of the seven files was checked against these reads: 62 rows, all Supported, none marked `H`.

## Rejected hypotheses and observations mined

No promotion. Each dismissal checked:

- R4: H1 to H7 hold (PoK on `c[0]`, PoP on `q`, index order, PoAP gating, group id parity, identifier derivation, redacting `Debug`). O1 and O3 (contract accepts a commitment or `y` that Rust rejects) exclude only the sender, as written: no amplification like `F3-XC-100`. O2 is wrong on exclusion (see the head's item 1 below). O4 is A1.
- R5: H1 to H18 hold; H13 (rollover shares from an orphaned branch) is accepted design, since the orphaned group is generated by the same validators and the guard forest keeps any validly signed rollover. O1, O2, O4 to O7 are liveness, gas or A1 hygiene, bounded. O3 is in `F3-VAL-032`.
- R6: every rejected hypothesis holds (typo keys fail loudly, bindings and gas measured, no live-reorg trigger for `F3-VAL-062`). O1 (empty `oracles`) is Informational; O2 is defense in depth; O3 belongs to C3-CORE; O6 is settled under the head's item 2 below; O9 is in `F3-VAL-062`.
- L-crypto: R1 to R19 hold; R8 (verifying share forced to `x >= N`) is right that the victim's on-curve `y` lands and only the attacker is excluded. O1 to O4 are known or A1 hygiene. O5 and O6 independently describe `F3-VAL-031` and `F3-VAL-030`.

## Head's extra task

1. R4 O2 against R6 Q29 (late rollover confirmation): R6 is right, nobody is excluded. The validator abandons epoch N's attempt at the first block with `1 + block / blocks_per_epoch > N` (`consensus/epoch.rs:6-9`, `state/keygen.rs:938-961`), that is at `block >= N * blocks_per_epoch`, and `NewBlock` runs the rollover handler before the key generation timeouts (`state/mod.rs:443-458`), so the stale group's confirm deadline never fires. `proposeEpoch` reverts only from `rolloverBlock > block.number` (`Consensus.sol:395-397`) with `rolloverBlock = N * blocks_per_epoch` (`state/keygen.rs:1360-1366`): the same boundary. `staged == 0` holds because `_processRollover` clears the previous stage first (`Consensus.sol:361-387`). Detail R6 missed, liveness only: a last confirmation mined in the block just before `rolloverBlock` starts rollover signing whose completing share reverts in `stageEpoch`; the share-round restart rule then retries with one signer fewer each round (`state/sign.rs:609-669`) until fewer than the threshold remain, costing gas for up to three rounds with the Aegis set (5 signers, threshold 3).
2. R4 Q30 against R6 O6 (late complaints): both are partly right. Counting matches the contract (R4). R6's outcome is real: a complaint mined after the local complain deadline needs no response (`state/keygen.rs:704-710`), a response to it is ignored (`:836-842`), and onchain the plaintiff cannot confirm with an open complaint (`FROSTParticipantMap.sol:220-223`), so the late plaintiff is excluded at the confirm deadline and the accused is not. Not promoted: the rule is a deliberate trade-off (the comment at `:704-707`) that keeps a last-minute complainer from forcing an honest accused past its response deadline, honest-to-honest complaints cannot occur, and the plaintiff must itself be about 120 blocks late. The same exclusion follows for any validator that misses the complaint window while offline: its complaint expires before it is sent (`:337-340`). Observation, Informational: a malicious participant can turn an honest validator's outage of roughly 10 to 30 minutes during the share phase into an exclusion for that epoch by sending it one invalid share (at most `threshold - 1` targets per round, or it becomes compromised itself).
3. O-COV-1 (`blocks_per_epoch`): folded into `F3-VAL-061` as an added trigger (same class and fix site; written in my Critic section there). Traced: a divergent node starts key generation at other blocks and for other epoch numbers, so it opens its own group and is excluded; two such nodes skip every Aegis epoch. If its epoch numbers coincide and it confirms last, its callback proposes another `rolloverBlock`, which the honest active group ignores until the `WaitingForRequest` timeout restarts signing with the honest message (`state/sign.rs:513-551`; `Action::Sign` calls the coordinator directly, `service/action.rs:272-289`), so it recovers after one `signing_timeout`. Timeout parameters are the same class; not traced.

## Commands

| Command | Result |
| --- | --- |
| `git rev-parse HEAD`; `git status --short -- crates contracts/src docs scripts` | `361aaf3`; clean |
| `git show fe9e84c:crates/validator/src/state/sign.rs` (lines 185-200) | pre-existence of the immediate approval reaction |
| `git log -1 7fc1e08` | #1025 message says the contract accepts `y = (0,0)` |
| `grep -n` over the maintainers' review notes and the team's decisions for 1025 and 1033 | the note from the review of #1025 open (Medium); the doc half of the note from the review of #1033 known |
| `sed -n 125,150p` of `frost-core-3.0.0/src/round2.rs` (registry) | `IncorrectCommitment` check confirmed |
| the style check on the seven finding files | 0 errors, 0 warnings |
| `explorer/node_modules/.bin/prettier --check` (3.9.6) on the seven files | all clean except `F3-VAL-001.md`, whose flagged lines (3-11, 22, 43) are R4's earlier text |

One `python3 -I` heredoc edited the three padded header cells of `F3-VAL-001.md` (each replacement asserted to match once), and one read-only `for` loop checked file endings; the brief lists heredocs and loops as guarded, so I switched to the Edit tool afterwards. No cargo, forge or Anvil run; nothing left under `scratch/`.

## Could not settle

- Gnosis reorg-depth frequency for the `F3-VAL-030` practicality check (no live RPC).
- Whether the Gnosis execution clients re-inject transactions from dropped blocks (inference; not needed for the guard path).
- Divergent `key_gen_timeout` or `signing_timeout` across validators (named as the same class as `F3-VAL-061`, not traced).
