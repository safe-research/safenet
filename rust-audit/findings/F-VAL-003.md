# F-VAL-003 A DKG complaint compels a plaintext share reveal with no check that the plaintiff ever received a share, no per-plaintiff bound and no deadline in the sharing round

| Field | Value |
| --- | --- |
| Status | QA-done |
| Crate and module | validator, state/keygen.rs |
| Location | crates/validator/src/state/keygen.rs:654-757 (related: crates/validator/src/state/keygen.rs:275-427, crates/validator/src/state/keygen.rs:1070-1108, crates/validator/src/frost/keygen.rs:417-428) |
| Severity | Medium / Medium |
| Certainty | 80% (Critic C-VAL-A; QA may raise) |
| Assumptions involved | A2, A7, A10 |
| Tags | crypto, input-validation, dos |

## Claim

`handle_key_gen_complained` treats a complaint as unconditional grounds to publish a secret. When this validator is the accused it queues `KeyGenComplaintResponse` carrying the plaintext scalar `f_me(id_plaintiff)` (`state/keygen.rs:734-745`, `frost/keygen.rs:420-428`) after checking only that (a) the event's group id matches, (b) this validator is participating, and (c) the local per-**accused** complaint counter is still below the threshold. Three checks that a dispute protocol would normally make are absent:

1. **No check that the plaintiff could have received a share.** The handler never consults `public_keys` or `shares`. A participant that has published nothing at all — and in the `CollectingShares` arm is not required to have — can open a dispute about a share it never had to process, and every accused answers it.
2. **No bound on complaints per plaintiff.** The threshold at `state/keygen.rs:722` counts complaints **per accused** (`complaints.entry(event.accused)`), and the contract's only limit is one complaint per `(plaintiff, accused)` pair (`FROSTParticipantMap.sol:181-183`). One participant may therefore open `n-1` disputes, one against each peer, and every one of them is answered with a plaintext secret while no counter approaches `threshold`.
3. **No deadline in the sharing round.** The `RolloverState::CollectingShares` arm (`state/keygen.rs:661-675`) matches on `group.id == event.gid` alone. The corresponding `CollectingConfirmations` arm does gate on `block <= deadlines.complain` (`685-695`), so the omission is visibly asymmetric rather than deliberate.

The direct effect is that any single registered participant can, at a time of its choosing during the sharing round, force all `n-1` other validators to broadcast plaintext DKG secret material on a public chain — turning a dispute-resolution mechanism into an on-demand disclosure primitive. This is the amplifier that makes **F-VAL-001** work: there, the same `n-1` complaints convert into the pads needed to reconstruct an honest participant's entire signing share. Fixing the ECDH pad (F-VAL-001/F-VAL-002) removes that consequence but leaves the policy defect: the complaint flow still publishes `f_X(id_M)` for every `X` on demand, still costs every honest validator a transaction per complaint, and still has no rule that would let an operator or a monitor distinguish "a peer is genuinely broken" from "a peer is harvesting".

## Basis

| # | Claim | Class (E1, E2, I) | Citation | Verbatim quote |
| --- | --- | --- | --- | --- |
| 1 | The `CollectingShares` arm gates on the group id only — no deadline, and no reference to `public_keys` or `shares` (both are skipped by the `..` rest pattern) | E2 | `crates/validator/src/state/keygen.rs:661-675` | <pre> match &mut state.rollover {<br> RolloverState::CollectingShares {<br> next_epoch,<br> group,<br> participation,<br> complaints,<br> deadline,<br> ..<br> } if group.id == event.gid => {<br> let restart_deadline =<br> deadline.map(\|_\| block.saturating_add(self.config.key_gen_timeout.get));<br> // We get at least another `key_gen_timeout` to get the<br> // complaint response onchain, which ends up being the same<br> // value as the restart deadline (by coincidence).<br> let response_expires_at = restart_deadline;</pre> |
| 2 | The confirmation arm _does_ carry a deadline guard, showing the sharing arm's omission is an asymmetry | E2 | `crates/validator/src/state/keygen.rs:685-701` | <pre> RolloverState::CollectingConfirmations {<br> next_epoch,<br> group,<br> participation,<br> complaints,<br> deadlines,<br> ..<br> } if group.id == event.gid<br> && deadlines<br> .as_ref<br> .is_none_or(\|deadlines\| block <= deadlines.complain) =><br> {<br> let restart_deadline = deadlines<br> .as_ref<br> .map(\|_\| block.saturating_add(self.config.key_gen_timeout.get));<br> let response_expires_at =<br> deadlines.as_ref.map(\|deadlines\| deadlines.response);</pre> |
| 3 | The abort threshold is per accused, so `n-1` complaints from one plaintiff against distinct accused never trip it | E2 | `crates/validator/src/state/keygen.rs:714-722` | <pre> let complaint = complaints.entry(event.accused).or_default;<br> complaint.total += 1;<br> complaint.unresponded += 1;<br><br> // If we ever get threshold complaints, the keygen is done. This is<br> // because it would reveal sufficient public information to compute<br> // secret key shares from one or more participants.<br> let (_, threshold) = group.size;<br> if complaint.total >= threshold {</pre> |
| 4 | The response is queued with no further validation of the plaintiff | E2 | `crates/validator/src/state/keygen.rs:734-745` | <pre> if let KeyGenParticipation::Participating(sharing_state) = participation<br> && event.accused == self.account<br> {<br> match frost::keygen::reveal_secret_share(sharing_state, event.plaintiff) {<br> Ok(secret_share) => {<br> commands.push(Command::Action(Action::KeyGenComplaintResponse {<br> group_id: group.id,<br> plaintiff: event.plaintiff,<br> secret_share,<br> expires_at: response_expires_at,<br> }));<br> }</pre> |
| 5 | The revealed value is the raw secret scalar this validator computed for the plaintiff | E2 | `crates/validator/src/frost/keygen.rs:420-428` | <pre>pub fn reveal_secret_share(sharing_state: &SharingState, peer: Address) -> Result<U256, Error> {<br> let identifier = participants::identifier(peer);<br> sharing_state<br> .peer_packages<br> .get(&identifier)<br> .map(\|package\| marshal::solidity_scalar(&package.signing_share.to_scalar))<br> .ok_or(frost_secp256k1::Error::UnknownIdentifier)<br> .err_unexpected<br>}</pre> |
| 6 | The contract permits complaints throughout `SHARING`, i.e. before the plaintiff has shared, and requires only that the plaintiff is `REGISTERED` (A7, reference only) | E2 | `contracts/src/FROSTCoordinator.sol:476-480` | <pre> function keyGenComplain(FROSTGroupId.T gid, address accused) external returns (bool compromised) {<br> Group storage group = $groups[gid];<br> GroupState memory state = group.state;<br> require(state.status == GroupStatus.SHARING \|\| state.status == GroupStatus.CONFIRMING, GroupNotReady);<br> compromised = group.participants.complain(msg.sender, accused) >= state.threshold;</pre> |
| 7 | The contract's only per-party limit is one complaint per `(plaintiff, accused)` pair, so `n-1` complaints from one plaintiff are legal (A7, reference only) | E2 | `contracts/src/libraries/FROSTParticipantMap.sol:181-183` | <pre> function complain(T storage self, address plaintiff, address accused) internal returns (uint16 totalAccusations) {<br> require(plaintiff != accused, InvalidParticipant);<br> require(self.complaints[plaintiff][accused] == ComplaintStatus.NONE, AlreadyComplained);</pre> |
| 8 | The contract does not verify the revealed scalar either, so the entire adjudication is offchain (A7, reference only) | E2 | `contracts/src/FROSTCoordinator.sol:494-500` | <pre> function keyGenComplaintResponse(FROSTGroupId.T gid, address plaintiff, uint256 secretShare) external {<br> Group storage group = $groups[gid];<br> GroupStatus status = group.state.status;<br> require(status == GroupStatus.SHARING \|\| status == GroupStatus.CONFIRMING, GroupNotReady);<br> group.participants.respond(plaintiff, msg.sender);<br> emit KeyGenComplaintResponded(gid, plaintiff, msg.sender, secretShare);<br> }</pre> |

## Trigger

In a group of `n` participants during the secret-sharing round (contract status `SHARING`, local state `RolloverState::CollectingShares`):

1. Honest participants publish `keyGenSecretShare`. A registered participant `M` publishes nothing.
2. `M` calls `keyGenComplain(gid, X)` once for each of the other `n-1` participants `X`. Every call is accepted: `FROSTCoordinator.sol:479` allows complaints in `SHARING`, and `FROSTParticipantMap.complain` requires only that `M` is `REGISTERED` (it committed in round 1) and that this exact pair has not complained before.
3. Each honest `X` matches `state/keygen.rs:661-675` — no deadline, no check on whether `M` has shared — increments `complaints[X].total` to `1` (below `threshold >= 2`, so `722` does not fire) and queues `keyGenComplaintResponse(gid, M, f_X(id_M))` at `734-745`.
4. All `n-1` plaintext scalars appear in `KeyGenComplaintResponded` events.

`M` pays `n-1` complaint transactions; the honest validators pay `n-1` response transactions between them (fixed gas 300k each, `crates/validator/src/service/action.rs:184` and `:233` — R6's file, cited for cost only) and publish `n-1` secrets. `M` is subsequently excluded on the share-collection timeout (`state/keygen.rs:1047-1068`) — but only after the disclosure has already happened, and the ceremony restarts under a new group id (`1188-1225`), where `M`'s `(plaintiff, accused)` pair counters reset and step 2 can be repeated.

## Considered and rejected

- **"The per-accused threshold bounds the disclosure."** It does not. `complaints` is keyed by `event.accused` (`state/keygen.rs:714`), so `n-1` complaints spread over `n-1` distinct accused leave every counter at `1`. The threshold is `count/2 + 1 >= 2` (`consensus/group.rs:220-222`, and `FROSTCoordinator.sol:347` enforces `threshold > 1`), so a single complaint per accused can never reach it.
- **"The contract bounds complaints."** Only per pair (`FROSTParticipantMap.sol:183`). There is no per-plaintiff cap anywhere.
- **"An accused could just not respond."** Refusing is worse for the accused than responding: `handle_key_gen_timeouts` excludes every participant with an unresponded complaint once `deadlines.response` passes (`state/keygen.rs:1078-1086`), and the contract blocks the _plaintiff_ from confirming while its own complaint is open (`FROSTParticipantMap.sol:219-222`). The unconditional response at `734-745` is the only behaviour that keeps an honest validator in the group, which is exactly why the missing plaintiff-side checks matter.
- **"What is disclosed is only the attacker's own shares, which it could decrypt anyway."** True, and it is why this is Medium rather than High **once the pad is fixed**. As the code stands the disclosure is what converts a copied `q` into full recovery of an honest participant's signing share — see F-VAL-001, which depends on claims 1, 3 and 4 here. The two findings should be read together; this one survives the F-VAL-001 fix, and F-VAL-001 is much harder to mount if this one is fixed too.
- **VAL-H9 (late complaints exclude the plaintiff), considered and largely refuted.** The hypothesis in `rust-audit/analysis/analysis-validator.md:335-341` is that a complaint filed after `deadlines.complain` is ignored offchain, the accused never responds, and the plaintiff is excluded at the confirm deadline. The mechanism is real — the guard at `685-695` returns early at `711`, and `handle_key_gen_timeouts` then falls through the empty-`unresponded` branch to `exclude_all_others(confirmations)` at `1087-1090` — but it **excludes the plaintiff, not the accused**, because the contract blocks a plaintiff with an open complaint from confirming (`FROSTParticipantMap.sol:222`). It is therefore self-defeating for an attacker. An _honest_ late plaintiff is largely protected by the action expiry: the complaint is queued with `expires_at = block + key_gen_timeout` at the block the bad share was seen (`state/keygen.rs:339-346`), which is at or before `deadlines.complain = last_share_block + key_gen_timeout` (`373-379`), so a complaint that would land late should expire instead. Not filed; recorded in `../state/coverage-logs.md#r4`.
- **"An honest node could diverge on the deadline."** No: `deadlines` are derived from the block number of the event that closed the share round (`state/keygen.rs:373-379`), which every honest node reads identically from the chain.
- **Membership of `event.plaintiff` / `event.accused`.** The handler inserts `event.accused` into `complaints` without checking `group.participants.contains(&event.accused)` (`714`), and `reveal_secret_share` is called with an unvalidated `event.plaintiff` (`737`). Under A7 the contract guarantees both are registered members (`FROSTParticipantMap.sol:181-187`), and `reveal_secret_share` fails closed for a non-peer (`frost/keygen.rs:424-427` returns `UnknownIdentifier`, logged at `746-753`, no reveal). So this is not independently exploitable; it is listed as a hardening item under Remediation and as observation O7 in the coverage log because it becomes reachable if VAL-H2 (address-agnostic event dispatch, R6's scope) holds.

## Remediation options

1. **Refuse to answer a complaint from a participant that has not published a share.** In the `CollectingShares` arm, carry `public_keys` out of the pattern (it is currently discarded by `..` at `state/keygen.rs:668`) and skip the reveal when `!public_keys.contains_key(&event.plaintiff)`. A plaintiff that genuinely cannot decrypt is still able to publish a share first — the share it publishes does not depend on what it received — so this costs an honest disputant nothing, while forcing an attacker to commit to a share before it can harvest. Cheapest change with the largest effect on F-VAL-001.
2. **Bound complaints per plaintiff and treat an excess as misbehaviour.** Track `complaints_by_plaintiff: BTreeMap<Address, u16>` alongside the per-accused map and restart the ceremony excluding a plaintiff that exceeds a small bound (one or two; an honest node should rarely see more than one broken peer, and if it does the per-accused threshold will fire anyway). This directly caps the disclosure a single party can compel at one secret rather than `n-1`.
3. **Add the missing deadline to the sharing arm.** Gate the `CollectingShares` branch on the same clock the confirmation branch uses, so complaints are only accepted once the share round has actually closed. This alone forces an attacker to publish its share before it can complain, which (combined with the fact that it cannot produce valid ciphertexts) makes F-VAL-001's ordering impossible. Tradeoff: an honest node that spots a bad share early must hold the complaint until the round closes, costing up to one `key_gen_timeout` of latency; the action's `expires_at` already accommodates that (`339-340`).
4. **Defence in depth: validate membership in every DKG event handler.** Check `group.participants.contains(&…)` for `event.participant` in `handle_key_gen_committed` (`173-175`) and `handle_key_gen_confirmed` (`448`), and for `event.accused` in `handle_key_gen_complained` (`714`). These are guaranteed by the contract today (`FROSTParticipantMap.sol:146-150`), so the checks are free insurance against event injection (VAL-H2) and against a future contract change.
5. **Consider a dispute mechanism that does not publish the share at all** — for example, the accused publishes a verifiable-decryption proof that the ciphertext it broadcast decrypts to a value consistent with its committed polynomial, rather than the value itself. Substantially more work and a contract change, but it removes the disclosure primitive entirely.

Tests to add. No code is committed.

- A state-machine test that `handle_key_gen_complained` emits no `KeyGenComplaintResponse` when the plaintiff is absent from `public_keys` in `CollectingShares`.
- A test that `n-1` complaints from one plaintiff against distinct accused trigger the restart path once the per-plaintiff bound is added (there is currently **no** test anywhere in `crates/validator/src/state/` — `grep -c '#\[test\]' crates/validator/src/state/keygen.rs` is 0).
- A test that a complaint arriving after the sharing round's deadline is ignored, pinning the intended asymmetry rather than leaving it implicit.

## Trail

- Reviewer R4: drafted, self-estimate 75%. All validator citations re-opened in this checkout at commit `2893917`; Solidity read as reference under A7. Class `E2` throughout — the missing checks are visible in the cited code and the trigger is a concrete, contract-accepted transaction sequence. `E1` unreachable this run (no toolchain, A9 FALSE). The 25% of doubt is about standalone severity rather than the mechanism: with the F-VAL-001/002 pad fix in place the compelled disclosure reveals only the complainant's own shares, so a Critic may reasonably place this at Low; I rate it Medium because it is the enabling step of a Critical finding and because "publish a secret on demand to anyone who asks" is a policy defect regardless of the current blast radius.

## Critic (C-VAL-A)

Read `state/keygen.rs:649-903` and `FROSTParticipantMap.sol` before R4's argument. My independent reading found the same three gaps and one additional fact R4 uses but does not state explicitly: `complaints` is keyed by `event.accused` while the contract's dedup key is the `(plaintiff, accused)` **pair**, so the two sides bound different quantities and neither bounds the one that matters — complaints _per plaintiff_.

### Per-claim verdicts

| # | Verdict | Note |
| --- | --- | --- |
| 1 | **Supported** | `state/keygen.rs:661-675` verbatim; the arm's guard is `group.id == event.gid` and nothing else, and `public_keys`/`shares` are absorbed by the `..` rest pattern as claimed. |
| 2 | **Supported** | `state/keygen.rs:685-701` verbatim; the confirmation arm's `block <= deadlines.complain` guard is real, so the asymmetry is genuine. |
| 3 | **Supported** | `state/keygen.rs:714-722` verbatim. |
| 4 | **Supported** | `state/keygen.rs:734-745` verbatim. |
| 5 | **Supported** | `frost/keygen.rs:420-428` verbatim; the lookup is `peer_packages[identifier(plaintiff)]`, populated by `dkg::part2` for every group member, so it succeeds for any member. |
| 6 | **Supported** | `FROSTCoordinator.sol:476-480` verbatim; and `FROSTParticipantMap.complain:185` requires `REGISTERED`, not "has shared", which is the operative point. |
| 7 | **Supported** | `FROSTParticipantMap.sol:181-183` verbatim. |
| 8 | **Supported** | `FROSTCoordinator.sol:494-500` verbatim; `respond` only flips the pair's `ComplaintStatus` and decrements two counters, and the scalar is emitted unexamined. |

Gas citation spot-checked: `service/action.rs:184` and `:233` both read `gas: 300_000,` as stated. No `H` claims.

### Corrections

**Correction 1 — sub-claim 1 of the Claim is close to vacuous as worded.** "No check that the plaintiff could have received a share" is not a real gap: every registered participant _is_ sent a share by construction, because `generate_secret_shares` encrypts one slot for every entry of `commitments` except the sender (`frost/keygen.rs:196-214`), and `commitments` is only complete when `commitments.len == count` (`state/keygen.rs:196-213`). The accused also has no way to adjudicate the complaint — a decryption failure at the plaintiff is not publicly verifiable — so an unconditional response is arguably forced by the design, not an oversight. The operative gap is the second half of the same bullet, which R4 does state: **the plaintiff is not required to have published its own share**, so it can dispute a round it has not yet participated in. That is the property F-VAL-001 consumes and the one a fix must remove. I would re-word sub-claim 1 rather than drop it; the finding does not depend on the vacuous reading.

**Correction 2 — the Trigger's repeatability sentence is wrong as written.** "the ceremony restarts under a new group id (`1188-1225`), where `M`'s `(plaintiff, accused)` pair counters reset and step 2 can be repeated" does not follow: the share-collection timeout restarts with `group.exclude_all_others(public_keys.keys)` (`state/keygen.rs:1067`), and `M` — which published nothing — is precisely one of the addresses excluded. `M` is **not** in the restarted group and cannot repeat step 2 there. The correct repeatability argument is a different one, and it is stronger: `handle_rollover_new_block` recomputes the participant set with `excluded: BTreeSet::new` (`state/keygen.rs:954-961`), so exclusions do not survive an epoch boundary and `M` is readmitted at the next rollover, where it can force the disclosure again. R4 records that fact elsewhere in its own log (rejected hypothesis "Rollover into a dishonest set") but does not connect it here. Net effect on severity: unchanged — one forced disclosure and one wasted DKG round per epoch, not per round.

**Correction 3 — the "no deadline" half is the weakest of the three gaps.** As argued in my critique of F-VAL-001, the sharing round's own deadline (`1047-1053`) already bounds the window, so adding a deadline guard to the `CollectingShares` complaint arm would change nothing an attacker cares about. It is worth fixing as consistency hygiene — the asymmetry with `685-695` is real and will confuse the next reader — but it should not be presented as the primary defect. The exception is genesis, where `deadline` is `None` end to end (`52-53`) and the window is genuinely unbounded; that is F-VAL-004's territory and the two findings compose there.

### Finding verdict

**Confirmed. Certainty 80%. Severity Medium / Medium (unchanged).**

Severity: as a standalone policy defect — assuming F-VAL-001 and F-VAL-002 are fixed — a single participant inside A2's fault bound can, once per epoch, force `n-1` honest validators to publish plaintext DKG scalars and pay a 300k-gas transaction each, and stall that epoch's ceremony for one `key_gen_timeout` before being excluded. The disclosed scalars are ones the plaintiff already holds and, with the pad fixed, expose nothing further; the rollover completes after the restart. That is Medium: deliberate, attacker-triggered, repeatable, but degrading rather than breaking. It would be High only if the disclosure reached `threshold` evaluations of one honest polynomial, which the per-accused counter at `722` does prevent.

Certainty 80% rather than higher: all eight claims are `E2` and re-verified, but two elements of the trigger are reasoned rather than executed — that `M` can withhold its share for the whole harvest without a peer's local state diverging, and the per-epoch repeatability in Correction 2, which depends on the config-driven participant recomputation behaving as `954-961` reads. Neither is doubtful; neither has been run.

**To reach the 90s**, QA should drive an Anvil group through: `M` commits, honest peers share, `M` files `n-1` complaints, and assert (a) `n-1` `KeyGenComplaintResponded` events carrying distinct non-zero scalars, (b) no `compromised` flag, (c) `M` excluded on the share timeout, and (d) `M` present again in the _next_ epoch's participant set — (d) being the assertion that settles Correction 2.

## QA (QA-VAL)

**Outcome: Not attempted (no toolchain).** Certainty unchanged at **80%**; severity Medium / Medium unchanged.

**No dedicated PoC directory.** The finding's three suggested tests are state-machine tests over `handle_key_gen_complained`, and the harness they need is the one written for F-VAL-004 ([`rust-audit/poc/F-VAL-004/genesis_stall.rs`](../poc/F-VAL-004/genesis_stall.rs) — a `ValidatorConfig`, a `Transition` and synthetic `Coordinator` event logs, in a `#[cfg(test)]` child of `crate::state`). I did not write a fourth copy of it for this finding because the assertion it needs sits on a state value — `RolloverState::CollectingShares { participation: Participating(_), public_keys, .. }` — that requires a completed round-2 `SharingState`, i.e. a full three-party DKG ceremony threaded through the state machine. That is a substantial harness and it is the same one the F-VAL-001 Anvil flow test needs; building it once, for both, is the right order of work.

What that harness must assert, precisely:

1. `handle_key_gen_complained` in `CollectingShares` with `event.accused == self.account` and a `event.plaintiff` **absent from `public_keys`** emits **no** `Action::KeyGenComplaintResponse`. Fails today; passes under remediation option 1.
2. `n-1` complaints from one plaintiff against `n-1` distinct accused leave every `complaints[accused].total == 1`, so `state/keygen.rs:722` never fires. Passes today — that is the defect, and pinning it is what makes option 2 testable.
3. A complaint arriving after the share round's deadline is ignored. Fails today (the `CollectingShares` arm matches on `group.id` alone, `:661-675`); passes under option 3.

### Remediation check

**Option 1 (refuse to answer a plaintiff absent from `public_keys`) is sound and is the one to take first.** C-VAL-A's Correction 1 on F-VAL-001 is right that this, not the deadline, is the load-bearing half. The reviewer's cost argument is also right and worth keeping in the ticket: the share a plaintiff publishes does not depend on what it received, so an honest disputant loses nothing by having to share first.

One correction to the option as written. It is only meaningful in `CollectingShares`. By the time the group reaches `CollectingConfirmations` every participant has published a share by construction — `keyGenSecretShare` leaves `SHARING` only when `--state.pending == 0` (`contracts/src/FROSTCoordinator.sol:420-435`) — so `public_keys` contains everyone and the check is vacuous. The confirming-round complaint path (`state/keygen.rs:676-712`) is therefore **not** covered by option 1, and the per-plaintiff cap of option 2 is the only thing that bounds a harvest filed there. Implement option 2 with a key on `(plaintiff)` counted across _both_ rounds, or option 1 alone will read as complete when it is not.

**Option 2 (per-plaintiff bound) is sound.** The bound of one or two is defensible for the reason given. Note it needs a new `BTreeMap` in a snapshotted state variant, so it is a serialisation change to `RolloverState::CollectingShares` and `::CollectingConfirmations` — cheap, but it will invalidate existing snapshots, which matters for a rolling upgrade.

**Option 3 (add the missing deadline to the sharing arm) is sound but is hardening, not a fix,** and the finding should say so more plainly than it does. C-VAL-A established this on F-VAL-001: the share round's own `key_gen_timeout` already bounds how long an attacker may withhold, and the whole harvest fits inside it, so gating the complaint arm on `block <= deadline` does not stop the attack. Worse, the option's own claim that it "makes F-VAL-001's ordering impossible" is **wrong**: it rests on the attacker being unable to produce valid ciphertexts, which is exactly what the harvest gives it. Take option 3 for tidiness; do not count it as a mitigation.

**Option 4 (validate group membership in every DKG handler) is sound and free.** I re-checked that the contract does enforce it today (`FROSTParticipantMap.register` requires a Merkle proof of the address, `:146-153`), so these are insurance rather than live fixes, exactly as the option says.

**Option 5 (verifiable decryption instead of revealing the share) is the correct long-term answer and its cost is understated.** It is not only "a contract change": today `keyGenComplaintResponse` emits the scalar without verifying anything (`FROSTCoordinator.sol:490-497`) and every validator checks it offchain via `verify_revealed_secret_share`. Moving to a proof means the _verification_ has to move too, or the proof is just as unverified onchain as the scalar is now. Scope it as a protocol change, not a handler change.

**An adjacent gap none of the options names.** `FROSTParticipantMap.respond` decrements the complaint counter on _any_ response, valid or not — the contract never checks the revealed scalar against the accused's commitment. So an accused that responds with garbage clears its onchain complaint and can still `keyGenConfirm`, while the Rust excludes it locally (`state/keygen.rs:842-859`). That divergence is the same class as **F-VAL-067** and should be fixed in the same change; whichever option is taken here, the onchain and offchain views of "responded" must be made to agree.

## Post-merge revalidation (RV-VAL)

**Verdict: STILL VALID.** Certainty and severity unchanged. Merge commit `a7f3915`, which merges `origin/main` and the Certora FROST audit fixes I-01..I-09. `crates/validator` is untouched by the merge, so this finding's mechanism is byte-identical.

The merge shifts `contracts/src/FROSTCoordinator.sol` by two documentation-only hunks (`9e41b49`: NatSpec on the `SignShared` event and on `signShare`). Every function this file quotes is byte-identical — only its address moved. Corrected citations:

| Old | New |
| --- | --- |
| `FROSTCoordinator.sol:476-480` (basis row 6, `keyGenComplain` permits complaints in `SHARING`) | **`:482-486`** |
| `FROSTCoordinator.sol:479` | **`:485`** |
| `FROSTCoordinator.sol:494-500` (basis row 8, `keyGenComplaintResponse`) | **`:500-506`** |
| `FROSTCoordinator.sol:490-497` | **`:496-503`** |
| `FROSTCoordinator.sol:420-435` | **`:426-441`** |
| `FROSTCoordinator.sol:347` (`threshold > 1`) | **`:353`** |
| `FROSTParticipantMap.complain:185` | unchanged — the file was not touched |

Nothing in the merge marks the plaintiff `COMPROMISED`, bounds complaints per plaintiff, or makes the contract verify the revealed scalar, so basis rows 6 and 8 and the offchain-adjudication conclusion all stand.
