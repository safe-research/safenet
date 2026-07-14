# Rust validator port review findings

Review target: `41593a6` (`[E 4]: E2E Test With Rust Validator`)

Compared implementations:

- Rust validator: [`crates/validator/`](../crates/validator/)
- TypeScript validator: [`validator/`](../validator/)

## Executive summary

The Rust validator is not ready to be treated as a drop-in production replacement for the
TypeScript validator. Its core protocol implementation is strong and contains several meaningful
security and maintainability improvements, but the review found multiple high-severity reliability
issues and several consensus behaviors that are not safe to mix with TypeScript validators without
a coordinated compatibility decision.

The most important deployment conclusions are:

- A coordinated "stop TypeScript, start Rust" migration after genesis can deadlock the validator
  set.
- Transient database or RPC failures can permanently strand DKG, nonce preprocessing, or block
  processing.
- Restart and reorg recovery can duplicate non-idempotent actions or fail because required
  snapshots, DKG secrets, or nonce trees have already been pruned.
- Under duplicate proposals, malformed DKG input, reorgs, or timeout recovery, Rust and TypeScript
  validators can track different groups, signature IDs, signer selections, or active signing
  phases for the same on-chain packet.

The port should not be approved for production replacement until the high-severity findings and the
restart-recovery findings are resolved and tested. A mixed Rust and TypeScript deployment should
also be considered unsupported until the consensus inconsistencies below are either backported to
TypeScript, removed from Rust, or explicitly activated through a coordinated protocol version.

## Rust and TypeScript consensus inconsistencies

### Compatibility model

For this review, a consensus inconsistency is any implementation difference that can make two
honest validators process the same canonical chain and then disagree about one of the following:

- Which DKG group or epoch is being built.
- Which transaction or rollover packet is eligible for signing.
- Which FROST signature ID and signer selection is current.
- Which participant should initiate a retry or fallback.
- Whether an on-chain proposal should be signed, declined, retried, or forgotten.

Not every difference is a defect in the Rust implementation. Several are deliberate Rust fixes for
TypeScript soundness or liveness problems. They are nevertheless mixed-deployment incompatibilities:
if one half of a validator group applies the fix and the other half retains the old behavior, the
group can split its signature shares across different ceremonies or make different DKG decisions.

The inconsistencies fall into three classes:

| Class         | Meaning                                                                                                      | Deployment implication                                                               |
| ------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ |
| State fork    | Implementations retain different logical state after the same event history.                                 | Must be aligned before mixed deployment.                                             |
| Recovery fork | Honest-path state agrees, but timeout, restart, or reorg recovery produces different actions or signer sets. | Requires identical timeout configuration and failure tests before mixed deployment.  |
| Policy fork   | Implementations return different approve or decline decisions for the same packet.                           | The implementation majority determines policy; this must be versioned or eliminated. |

Several of the Rust-side divergences below are already accepted project decisions rather than
open questions: the stricter DKG validation timing, the genesis halt (no genesis ceremony
restart), the aggressive cleanup on observed attestations, and the eager
`signature_id_to_message` bookkeeping. For those, the alignment items in this document describe
a known fork that still needs a deployment decision (backport or coordinated cutover); they are
not newly discovered defects in the Rust port.

### Consensus inconsistency summary

| Area                            | Rust behavior                                                                                        | TypeScript behavior                                                                                          | Mixed-validator risk                                                                           |
| ------------------------------- | ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| Reorg processing                | Rolls snapshots back and replays canonical blocks and logs.                                          | Logs the uncle but explicitly does not roll state back.                                                      | Direct state fork after any relevant orphaned event.                                           |
| Duplicate transaction proposals | Keeps the existing signing phase and signature ID.                                                   | Replaces the signing state and follows the newest `Sign` event.                                              | Shares split across old and new signature IDs.                                                 |
| Duplicate oracle proposals      | Keeps the existing signing phase and signature ID.                                                   | Replaces the signing state and follows the newest request.                                                   | Same split as plain proposals, plus repeated oracle requests.                                  |
| DKG public-share validation     | Validates public verification shares against commitments and refuses invalid public input.           | Does not validate the posted public share when observing; trusts on-chain share completion.                  | Validators can disagree whether DKG entered confirmation or must exclude a participant.        |
| DKG completion source           | Counts locally verified commitments and public shares.                                               | Uses the contract's `committed` and `shared` booleans after partial local processing.                        | Malformed input can move TypeScript ahead while Rust remains in an earlier phase.              |
| Malformed curve-point handling  | Rejects the individual DKG or nonce event and continues with later logs.                             | Point-schema conversion throws before transition dispatch and aborts the remainder of the watcher log batch. | Later valid events in the same block or warp page are permanently observed only by Rust.       |
| Initial signing timeout         | Drops `WaitingForRequest` when responsibility is already “everyone.”                                 | Has every validator submit `sign` once, then drops the request.                                              | Rust cannot recover the same missing `Sign` event that TypeScript recovers.                    |
| Responsible signing timeout     | Excludes the failed responsible signer and has every remaining Rust validator submit immediately.    | Gives the responsible validator one retry window, then makes everyone responsible.                           | Different retry timing creates multiple signature IDs and signer sets.                         |
| Attestation fallback            | Every Rust validator submits one direct fallback at the first timeout.                               | The last signer retries first; everyone retries only at the second timeout.                                  | Extra ceremonies/gas and different cleanup timing.                                             |
| Action expiration clock         | Expires queued protocol actions at their block deadline.                                             | Keeps every queued action live for a fixed ten minutes of wall-clock time.                                   | TypeScript can submit an obsolete `sign` or share after Rust has expired the phase.            |
| Oracle attestation fallback     | Supports direct `attestOracleTransaction`.                                                           | Builds no direct oracle fallback in `waiting_for_attestation`.                                               | Rust can complete a callback failure that TypeScript eventually abandons.                      |
| Signature-share timeout         | Attributes participation per selection root and chooses a threshold root.                            | Counts shares across all selection roots.                                                                    | Retry signer sets diverge under competing or malicious roots.                                  |
| Expected signing group          | Stores the expected group and rejects a `Sign` event for a different group.                          | Checks only that the local validator belongs to the event's group and that the message was verified.         | A wrong-group `Sign` can divert TypeScript recovery state but not Rust state.                  |
| Late old-epoch signing          | Tops up the group named by the `Sign` event, including retained old groups.                          | Tops up only the currently active epoch group.                                                               | TypeScript can run out of usable nonces during an old-epoch retry while Rust continues.        |
| Pending nonce preprocessing     | Processes a nonce-tree link only for this validator's own event.                                     | Clears the group's local pending flag when any participant's `Preprocess` event lands.                       | A peer event can make TypeScript generate duplicate local nonce trees while Rust does not.     |
| Attestation cleanup             | Removes the packet state when an attestation lands, even if it completed under another signature ID. | Cleans up only from `waiting_for_attestation`.                                                               | TypeScript can continue retrying a packet that another ceremony already attested.              |
| Genesis failure                 | Halts when genesis would need a different subgroup.                                                  | Attempts to restart genesis with a new group ID.                                                             | Local state diverges, although the replacement group is not the authorized genesis group.      |
| First post-genesis DKG          | Starts immediately in the final-confirmation block.                                                  | Starts on the following block transition.                                                                    | If confirmation is the last block of an epoch, Rust and TypeScript can target adjacent epochs. |
| Transaction policy              | Rejects delegate-call entries inside `MultiSendCallOnly`.                                            | Applies the ordinary MultiSend delegate-call allow-list to CallOnly deployments.                             | Same Safe transaction is declined by Rust and accepted by TypeScript.                          |
| Policy operation routing        | Approves a plain `CALL` to a MultiSend, migration, `SignMessageLib`, or `CreateCall` address.        | Routes checks by `to` address before the operation and declines the same `CALL`.                             | Same Safe transaction is approved by Rust and declined by TypeScript.                          |
| Default consensus timing        | Defaults to 1,440 blocks/epoch, 6-block signing timeout, and 12-block oracle timeout.                | Defaults to 17,280, 120, and 120 respectively.                                                               | Immediate hard split at rollover or whenever a phase crosses the shorter Rust timeout.         |

### Implementation cross-reference

The principal comparison points are:

| Concern                                    | Rust                                                                                                                                                                                 | TypeScript                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Proposal and attestation lifecycle         | [`state/transactions.rs`](../crates/validator/src/state/transactions.rs)                                                                                                             | [`transactionProposed.ts`](../validator/src/machine/consensus/transactionProposed.ts), [`oracleTransactionProposed.ts`](../validator/src/machine/consensus/oracleTransactionProposed.ts), [`transactionAttested.ts`](../validator/src/machine/consensus/transactionAttested.ts), [`oracleTransactionAttested.ts`](../validator/src/machine/consensus/oracleTransactionAttested.ts) |
| DKG share validation and phase advancement | [`state/keygen.rs`](../crates/validator/src/state/keygen.rs), [`frost/keygen.rs`](../crates/validator/src/frost/keygen.rs)                                                           | [`secretShares.ts`](../validator/src/machine/keygen/secretShares.ts), [`timeouts.ts`](../validator/src/machine/keygen/timeouts.ts)                                                                                                                                                                                                                                                 |
| Signing state and timeout recovery         | [`state/sign.rs`](../crates/validator/src/state/sign.rs)                                                                                                                             | [`sign.ts`](../validator/src/machine/signing/sign.ts), [`timeouts.ts`](../validator/src/machine/signing/timeouts.ts), [`shares.ts`](../validator/src/machine/signing/shares.ts)                                                                                                                                                                                                    |
| Epoch rollover and genesis handoff         | [`state/keygen.rs`](../crates/validator/src/state/keygen.rs), [`state/mod.rs`](../crates/validator/src/state/mod.rs)                                                                 | [`confirmed.ts`](../validator/src/machine/keygen/confirmed.ts), [`trigger.ts`](../validator/src/machine/keygen/trigger.ts), [`epochStaged.ts`](../validator/src/machine/consensus/epochStaged.ts)                                                                                                                                                                                  |
| Reorg processing                           | [`core/state/storage.rs`](../crates/core/src/state/storage.rs), [`core/state/mod.rs`](../crates/core/src/state/mod.rs), [`core/index/blocks.rs`](../crates/core/src/index/blocks.rs) | [`shared/watcher.ts`](../validator/src/shared/watcher.ts)                                                                                                                                                                                                                                                                                                                          |
| Safe transaction policy                    | [`consensus/checks/mod.rs`](../crates/validator/src/consensus/checks/mod.rs)                                                                                                         | [`service/checks.ts`](../validator/src/service/checks.ts), [`checks/multisend.ts`](../validator/src/consensus/verify/safeTx/checks/multisend.ts)                                                                                                                                                                                                                                   |

### 1. TypeScript does not roll consensus state back on reorg

The Rust indexer and state machine treat reorgs as first-class transitions: they restore the parent
snapshot and replay the new canonical branch. The TypeScript watcher handles
`watcher_update_uncle_block` by incrementing a metric and logging `Reorg detected, but currently not
supported`; it does not restore consensus or FROST state.

This difference is a direct state fork, not just a recovery-quality difference. For example, if an
orphaned block contains a `TransactionProposed`, `Sign`, `EpochStaged`, `KeyGenCommitted`, or
`Preprocess` event:

1. Rust removes the orphaned transition and processes the replacement branch.
2. TypeScript retains the orphaned rollover/signing state and locally generated protocol actions.
3. Replacement logs at the same block and log index are out of order from the TypeScript state
   machine's perspective and may be rejected or ignored.
4. The implementations can subsequently sign different packets, DKG groups, or signature IDs.

The TypeScript behavior is visible in
[`validator/src/shared/watcher.ts`](../validator/src/shared/watcher.ts); Rust rollback and replay live
under [`crates/core/src/state/`](../crates/core/src/state/) and
[`crates/core/src/index/`](../crates/core/src/index/).

Mixed operation is therefore unsafe on a chain where reorgs are within the stated threat model. A
mixed test that exercises only linear Anvil blocks cannot establish compatibility.

Required compatibility decision:

- Backport snapshot rollback and canonical replay to TypeScript before mixed deployment, or
- Perform a coordinated cutover in which no TypeScript validator remains in a signing group that
  includes Rust validators.

### 2. Duplicate proposals split the validator set across signature IDs

`Consensus.proposeTransaction` and `proposeOracleTransaction` can be called repeatedly until an
attestation exists. Each call emits a proposal and synchronously opens a new coordinator signing
ceremony with a fresh sequence and signature ID.

Rust intentionally preserves an existing signing entry keyed by the packet message. TypeScript
unconditionally applies a new `waiting_for_request` diff for the duplicate proposal. The resulting
event sequence is:

1. Proposal 1 opens signature ID `sid-1`; all validators begin processing it.
2. Proposal 2 for the same packet opens `sid-2` before `sid-1` completes.
3. Rust ignores the duplicate proposal and the unexpected `sid-2` `Sign` event, remaining on
   `sid-1`.
4. TypeScript replaces its state and follows `sid-2`.
5. A mixed group publishes nonce commitments and signature shares to different ceremonies.

Neither ceremony reaches threshold unless one implementation alone controls a threshold of the
group. This is a low-cost, externally triggerable mixed-deployment liveness failure. Oracle
proposals additionally repeat the external oracle request.

Rust's behavior fixes a real TypeScript denial-of-service issue documented in commits `2eabb7f` and
`4bc18b5`. The fix should be backported to TypeScript before operating a mixed group; reverting the
Rust behavior would reintroduce the original attack. Commit `1c79517` separately documents the
related cleanup fork after an attestation lands.

### 3. DKG validation can produce different groups and rollover states

Rust validates more public DKG information than TypeScript:

- Every commitment must be locally valid before it contributes to phase completion.
- A posted public verification share must match the participant's commitments.
- The number and shape of encrypted shares must be valid.
- Observers perform the public checks even when they cannot decrypt a share intended for another
  validator.

TypeScript validates commitments and decrypts/verifies its own encrypted share, but its transition
to confirmation is driven by the contract's `shared` boolean. An observer does not validate the
posted public verification share. `sharesFrom` also records every on-chain submitter regardless of
whether the public portion is locally valid.

A malicious participant can therefore post a contract-accepted share whose public verification
share does not match its DKG commitment:

1. Rust refuses to count that participant as having shared and remains in `CollectingShares`.
2. TypeScript can enter `collecting_confirmations` when the contract reports `shared = true`.
3. At timeout, Rust excludes the invalid participant and derives a new group ID.
4. TypeScript can continue confirming or signing a rollover for the original group.

If TypeScript validators stage the original group, Rust validators that rejected it do not record
the same epoch group. Their later rollover hashes and available signing keys can remain permanently
out of sync.

Rust's stronger validation is the correct security behavior and matches the rationale documented in
the port history. It is still a protocol behavior change and needs an adversarial mixed-validator
test plus a coordinated activation or TypeScript backport.

### 4. Malformed public points abort a TypeScript log batch but not Rust processing

The coordinator checks that DKG and nonce points are nonzero, but it does not establish that every
point is a valid, non-identity secp256k1 point before emitting it. In particular, a participant can
commit structurally valid but off-curve DKG data, or register a nonce-tree root containing
off-curve coordinates and later reveal that leaf.

Rust decodes the coordinates as contract data and performs curve validation inside the applicable
state transition. It rejects the individual commitment, public share, or nonce reveal and then
continues processing later logs.

TypeScript transforms every FROST point through `toPoint` in
[`transitions/schemas.ts`](../validator/src/machine/transitions/schemas.ts). An off-curve point
throws while `logToTransition` is being evaluated, before the watcher's per-transition error
boundary. The exception escapes the `for (const log of update.logs)` loop, and the watcher catches
it only at the outer handler boundary. Consequences are broader than rejecting the malicious
event:

1. Every valid log after the malformed one in that `watcher_update_new_logs` batch is skipped.
2. The underlying event watcher has already consumed the block or warp page and does not enqueue
   those later logs again.
3. Rust records the later valid DKG, nonce, share, and attestation events while TypeScript does not.
4. The implementations can exclude different participants, choose different retry signer sets, or
   remain in different phases permanently.

During historical catch-up, one update can cover a page containing logs from many blocks, so the
loss is not necessarily limited to the remainder of one block. This gives a malicious participant
who can publish a contract-accepted malformed point influence over unrelated later consensus
events.

Required remediation:

- Parse structural event data without throwing at the batch level.
- Validate curve points inside an event-local transition boundary.
- Continue processing later logs after recording an invalid event, or fail the entire update
  transactionally and replay it from the same cursor.
- Test a malformed event at the beginning, middle, and end of both a live block batch and a
  historical warp page.

### 5. Timeout defaults and responsibility rules create different ceremonies

The different defaults are consensus-affecting, not merely operational:

- Rust signing timeout: 6 blocks; TypeScript: 120 blocks.
- Rust oracle timeout: 12 blocks; TypeScript: 120 blocks.
- Rust blocks per epoch: 1,440; TypeScript: 17,280.

If a signing round lasts more than six blocks under defaults, Rust can abandon the current
signature ID and open a retry while TypeScript remains on the original ID for another 114 blocks.
When the retry `Sign` event lands, TypeScript is not in `waiting_for_request` and ignores it. The
mixed group is then split across two ceremonies.

An oracle result arriving after block 12 but before block 120 is similarly accepted by TypeScript
and ignored by Rust. A blocks-per-epoch mismatch is more severe: validators derive different DKG
targets and different `rolloverBlock` values, so they hash and sign different rollover packets.

Even with identical configured timeout values, the responsibility algorithms differ:

- In `WaitingForRequest` with a named responsible participant, Rust removes that participant and
  immediately has every remaining validator submit `sign`. TypeScript gives the named participant
  one retry window and only then lets everyone act.
- In `WaitingForRequest` with no named responsible participant, TypeScript has everyone submit once;
  Rust drops the request without an action.
- In `WaitingForAttestation`, Rust has everyone submit a direct fallback at the first timeout and
  then forgets the state. TypeScript retries through the last signer first and everyone second.

The implementations also disagree on when an already-queued action becomes stale. Rust attaches
the phase's block deadline to signing, DKG, and fallback actions, and the durable transaction queue
will not allocate a nonce after that block. TypeScript's `BaseActionQueue` gives every action a
fixed ten-minute wall-clock lifetime, independent of the configured block timeout. On a fast chain,
under an RPC outage, or with a short explicit timeout, TypeScript can submit an old `sign` action
after Rust has advanced to a retry generation. That stale call increments the coordinator sequence
and opens another signature ID.

The normal proposal path emits `Sign` synchronously, so the no-responsible recovery difference is
primarily exposed by missing logs, reorgs, or earlier processing failures. Those are exactly the
conditions the timeout state is intended to recover from.

Required compatibility decision:

- Remove consensus-affecting defaults from production configuration.
- Fail startup unless all epoch and timeout parameters are explicit.
- Define one responsibility state machine and implement it in both languages.
- Use the same block-based action validity rule in both transaction queues.

### 6. Oracle attestation recovery exists only in Rust

When `signShareWithCallback` completes the FROST signature but its consensus callback does not land,
Rust can build direct fallback actions for epoch rollover, plain transactions, and oracle
transactions.

TypeScript's `waiting_for_attestation` timeout builds direct actions only for epoch rollover and
plain transaction packets. An oracle packet falls through without an
`attestOracleTransaction` action. After its second timeout, TypeScript removes the signing state and
signature mapping without ever submitting the completed oracle attestation.

In a mixed group, Rust may successfully attest the oracle transaction while TypeScript abandons it.
The on-chain result ultimately converges, but the implementations differ in responsibility, retry
load, and their ability to make progress without the Rust subset.

The epoch-rollover fallback is also more fragile in TypeScript than in Rust. At the
`waiting_for_attestation` timeout, TypeScript only emits `consensus_stage_epoch` when the
rollover machine is still in `sign_rollover` for that exact message; Rust builds the direct
`stageEpoch` action from the packet itself. If the rollover state has moved on (for example
after a skip or restart), TypeScript abandons a staging that Rust still submits.

The Rust fallback should be backported to TypeScript, and callback-failure integration tests should
cover all three packet types.

### 7. Signature-share timeout participation is defined differently

The coordinator permits shares for multiple signer-selection roots within the same signature ID.
Only shares submitted to the same root can combine into a valid threshold signature.

Rust stores `shares_from` and `last_signer` separately for each selection root. On timeout it chooses
the unique threshold-sized root with the most shares and uses only those participants for retry.

TypeScript stores a single `sharesFrom` array without the selection root. A malicious participant
can submit a valid share under a different root, be counted as participating, and remain in the
retry signer set even though it did not contribute to the root the honest validators were building.

This was intentionally fixed in commit `ed53c8a`. In a mixed deployment, Rust and TypeScript can
derive different retry signer sets and different responsible validators from the same
`SignShared` event sequence. The TypeScript behavior should be backported rather than retained as a
permanent compatibility mode.

The implementations also track a timed-out ceremony differently when the local validator is not
part of the retry signer set (or the set falls below threshold): Rust drops the signing session
entirely, removing its signature-id mapping, while TypeScript re-enters `waiting_for_request`
and keeps observing regardless of its own membership. Subsequent retry events for the packet are
then processed by TypeScript and ignored by Rust.

### 8. Rust binds a verified packet to its expected group; TypeScript does not

Rust stores `group_id` in `WaitingForRequest` and only accepts a `Sign` event when `event.gid`
matches that expected group. TypeScript checks that the local validator belongs to `event.gid` and
that the message is verified, but it does not compare the event group with the packet epoch's
group.

The coordinator's `sign` function is public and can be called for any finalized group and arbitrary
nonzero message. If an old or future finalized group has the same participant addresses, a
wrong-group `Sign` for a valid pending packet can divert TypeScript from a recovery state into a
ceremony whose signature the consensus contract will not accept for that epoch. Rust ignores the
same event.

The initial proposal and its legitimate `Sign` are emitted atomically, which limits this attack on
the first attempt. It remains relevant during timeout/retry windows when the packet is again in
`WaitingForRequest`.

Rust's expected-group check should be added to TypeScript and covered by a direct coordinator-sign
test.

### 9. Old-epoch nonce availability differs during late retries

Any `Sign` call increments the sequence for the group named by the event. Rust locates the retained
epoch with that group ID and tops up its nonce stock, including a group that stopped being active
while a signing ceremony was still in progress.

TypeScript's top-up path always uses `consensusState.activeEpoch`, not `event.gid`. For a late retry
under the previous epoch, it can generate nonces for the new active group while leaving the old
group depleted. Rust validators continue participating; TypeScript validators eventually fail to
reveal an old-group sequence.

There is a second preprocessing bookkeeping mismatch. TypeScript's `groupPendingNonces[groupId]`
flag represents a locally generated tree, but `handlePreprocess` clears it when _any_ participant's
event for that group lands. If a peer's preprocessing transaction is ordered before this
validator's transaction, the next `Sign` can make TypeScript generate and enqueue another local
tree. Rust links a tree only when `event.participant` is its own account and otherwise leaves its
local work unchanged. This does not by itself change packet hashes, but it makes nonce inventory,
queued actions, and recovery behavior diverge.

This difference is intentional and correct in Rust. It should be backported before mixed operation
across epoch boundaries. TypeScript should also clear pending preprocessing only for its own
matching root.

### 10. Attestation cleanup differs after competing ceremonies

Rust removes the packet's signing state whenever the corresponding on-chain attestation lands,
even if the local state was following a different signature ID or had not yet reached
`WaitingForAttestation`.

TypeScript only cleans up if its current state is exactly `waiting_for_attestation`. If a Rust
majority completes `sid-1` while TypeScript followed `sid-2` after a duplicate proposal, TypeScript
ignores the attestation for cleanup and can continue retrying an already-attested packet. Its later
callbacks revert with `AlreadyAttested`, consuming nonce stock and gas.

The defensive Rust cleanup should be backported together with duplicate-proposal handling so both
implementations converge on the first valid on-chain attestation.

TypeScript has a smaller related leak when an oracle rejects a request or its response times out:
it removes the signing entry but leaves `signatureIdToMessage[sid]`. Rust removes both. Signature
IDs are not reused, so this alone is not a ceremony fork, but it is another case where the same
terminal event leaves different durable consensus state and routes late signature events through
different lookup results.

### 11. Genesis recovery and first-epoch timing differ

Rust treats a genesis DKG restart as unrecoverable because excluding a participant changes the
genesis group ID and the replacement is not the group authorized by deployment. TypeScript attempts
to trigger a replacement genesis subgroup. Rust's halt is the correct behavior, but the local state
machines diverge after a threshold complaint or equivalent genesis failure.

Fresh-node recovery also differs. If an `EpochStaged` event arrives while Rust is still
`WaitingForGenesis`, Rust moves to `EpochSkipped` so it can join a later DKG. TypeScript in
`waiting_for_genesis` ignores the same event; it only has equivalent catch-up behavior when the
operator starts it in the separate `SKIP_GENESIS` mode. This makes startup flags, not canonical
chain state, determine whether otherwise identical validators ever join the active rollover flow.

There is also an edge at successful genesis completion:

- Rust starts the first regular DKG immediately while handling the final `KeyGenConfirmed` event,
  targeting `floor(block / blocks_per_epoch) + 1`.
- TypeScript records genesis as staged and starts the first regular DKG on the next block
  transition, targeting `floor((block + 1) / blocks_per_epoch) + 1` when the next block is processed.

For most blocks both expressions yield the same epoch. If the final genesis confirmation is in the
last block before an epoch boundary, Rust targets epoch `N + 1` while TypeScript targets epoch
`N + 2`. Their group contexts and group IDs differ, so each implementation ignores the other's DKG
events.

Commit `837ccaa` describes the one-block difference as non-divergent, but the boundary case is not
covered. The implementations should use the same event block to derive the first post-genesis epoch,
and an integration test should finalize genesis immediately before a boundary.

### 12. Safe transaction policy has `MultiSendCallOnly` and operation-routing forks

Rust distinguishes ordinary MultiSend contracts from `MultiSendCallOnly` and rejects delegate-call
subtransactions for CallOnly addresses. TypeScript constructs the same subtransaction checker with
`allowedDelegateCalls` for both contract families.

The same proposal is therefore placed in `WaitingToDecline` by Rust and `WaitingForRequest` by
TypeScript. A mixed group signs or declines according to which implementation controls threshold.
Even if the CallOnly contract would later revert the unsupported subtransaction, the validator
network should not disagree about whether the proposal satisfies policy.

There is a second, independent fork in the check structure itself. TypeScript routes checks by
`to` address before considering the operation: a plain `CALL` (operation 0) to any configured
MultiSend, `MultiSendCallOnly`, `SafeMigration`, `SignMessageLib`, or `CreateCall` address
reaches a checker that requires `operation == 1` and declines the proposal (the
`checks/config` tests assert "Expected operation 1 got 0"). Rust checks the operation first
([`consensus/checks/mod.rs`](../crates/validator/src/consensus/checks/mod.rs)) and approves any
`CALL` to a non-self address. The same plain call to, for example, the MultiSend 1.3.0 address
is therefore `WaitingForRequest` in Rust and `waiting_to_decline` in TypeScript. TypeScript is
also internally inconsistent here: it allows the equivalent `CALL` when it appears as a
sub-transaction inside a MultiSend batch.

Two further differences are currently latent and should be closed with the same corpus:

- Rust zeroes `chainId` and `nonce` in decoded MultiSend sub-transactions
  ([`checks/multi_send.rs`](../crates/validator/src/consensus/checks/multi_send.rs)); TypeScript
  propagates both from the outer transaction. No check consumes either field today.
- TypeScript's `buildAddressSplitCheck` supports chain-scoped `eip155:<chainId>:<address>` keys
  with no Rust counterpart. No such key is configured today; the first one would fork policy
  silently.

Outside of these forks, the two policies were verified to agree: the six allow-lists (fallback
handlers, guards, modules, module guards, migration contracts, sign-message and create-call
libraries) contain identical addresses, selector matching semantics agree, and the MultiSend
byte decoding (including `to == 0` self-call mapping for v1.5.0+ and strict length handling) is
byte-for-byte equivalent.

Required remediation:

- Correct the TypeScript CallOnly checker and define one operation-routing rule for calls to
  policy-listed addresses.
- Build a shared policy corpus and assert identical approve/decline results in both languages,
  including plain calls (operation 0) to every policy-listed address.
- Treat any future policy change as a versioned consensus change, not a local implementation detail.

### Consensus behaviors already shown to be compatible

The comparison did not find a hashing or honest-path FROST wire-format mismatch in the following
areas:

- Genesis and regular group ID derivation for the same ordered participant set and epoch context.
- Majority signing threshold and greater-than-two-thirds minimum DKG participation formulas within
  the supported participant-count range.
- Safe transaction, oracle transaction, and epoch-rollover packet hashing.
- Participant identifiers, nonce commitment leaves, signer-selection leaves, and Solidity ABI
  encoding.
- Honest-path DKG commitments, encrypted shares, nonce reveals, and signature shares exercised by
  the mixed integration flow.
- DKG `f[]` share ordering (ascending participant address, excluding self) on both the publish
  and receive paths, and signer-selection ordering (both sort by the hashed FROST identifier).
- Merkle trees: sorted-pair Keccak hashing, zero-hash padding, and proof generation are
  byte-for-byte equivalent (TypeScript throws on an empty tree where Rust returns a zero root,
  which is unreachable with valid inputs).
- Keygen confirmation deadline structure (complain/respond/confirm at one/two/three keygen
  timeouts from share completion) and the complaint/response acceptance windows.
- Epoch activation timing (both implementations activate a staged epoch on the block clock at
  `epoch * blocks_per_epoch`, and both ignore an `EpochStaged` event for a group other than the
  one they generated) and the signing-session-aware cleanup cutoff for retired epoch groups.
- The Safe policy allow-list addresses and the fixed per-action gas constants (identical on both
  sides, so fixed gas is a shared operational risk rather than a divergence).

These compatibility results are important but do not cover adversarial events, timeouts, restarts,
or reorgs where the state-machine inconsistencies occur.

## High-severity findings

### 1. No viable live-network bootstrap or TypeScript migration path

The Rust process resolves the coordinator and optionally reconciles the staker at startup, but it
does not read the current consensus epoch, active or staged group, or rollover state from chain. See
[`crates/validator/src/main.rs`](../crates/validator/src/main.rs).

A new Rust database begins in `WaitingForGenesis`. With the default `start_block = None`, the
indexer only processes blocks in the recent reorg window. If neither genesis nor an `EpochStaged`
event is present in that window, the validator remains in `WaitingForGenesis` until a future staged
event is emitted.

When a validator in `WaitingForGenesis` eventually observes an `EpochStaged` event, it enters
`EpochSkipped` and only starts DKG for a later epoch. Depending on when the event is observed, the
validator may wait almost two full epochs before it has a usable key share.

Consequences:

- If all TypeScript validators are replaced simultaneously after genesis, no Rust validator starts
  the next rollover. The validator set can remain stuck indefinitely.
- A rolling migration can recover only while the remaining TypeScript validators retain enough
  quorum to stage future epochs.
- Replaying from block zero is not a migration solution. Historical DKG randomness cannot recreate
  an existing private key share, and old protocol actions cannot safely be reproduced.
- The TypeScript and Rust database and key-share formats have no migration or import path.

The TypeScript implementation has an explicit `SKIP_GENESIS` state that allows an instance to join
a running deployment without pretending that genesis will be observed again.

This gap is also inconsistent with commit `3784513`, which removed the Rust `skip_genesis` option
on the stated basis that the value would be derived from on-chain consensus state. That on-chain
bootstrap was not implemented.

Required remediation:

- Read the current epoch, rollover state, active group, and staged group from chain during startup.
- Define a safe state from which a new validator can join a future DKG.
- Provide and test private-share migration if a coordinated replacement is meant to preserve the
  active group. Otherwise explicitly prohibit big-bang migration and document a rolling migration
  procedure.
- Add cold-start tests for genesis, an active epoch, a staged epoch, and a deployment with no
  TypeScript validators remaining.

### 2. Critical effects fail once and are silently converted to `Noop`

Every Rust effect error is logged and converted into `Resume::Noop` in
[`crates/validator/src/service/effect.rs`](../crates/validator/src/service/effect.rs). The failed
effect is not retained or retried.

This is unsafe for state transitions that depend on one-shot effects:

- `KeyGenSetup` moves the state into `WaitingForSetup` before generating and persisting secrets.
- Keygen timeout handling covers commitment, share, and confirmation collection, but not
  `WaitingForSetup`.
- Genesis setup has no deadline, so one transient storage or randomness failure can strand genesis
  permanently.
- During a regular epoch, a setup failure strands the validator until rollover abandons that epoch.
- If `LinkNonceTree` fails, the associated `Preprocess` event has already been consumed. The
  unlinked chunk is nevertheless included in `available_nonce_count`, so nonce top-up sees adequate
  capacity and does not generate a replacement. The validator can permanently lose the ability to
  reveal the corresponding on-chain nonce commitments.

Commit `69f7ef8` introduced `WaitingForSetup` specifically to allow setup to be retried on later
block updates. The commit message states that the retry would be added when block handling landed,
but it was never implemented.

Required remediation:

- Return a typed failure resume instead of collapsing all failures to `Noop`.
- Persist retryable setup and link work in state.
- Retry setup and linking on subsequent blocks with bounded backoff.
- Keep a preprocessing root pending until its database link has been confirmed.
- Test transient failures before and after database writes so retries remain idempotent.

### 3. The driver consumes failed updates instead of retrying them

The watcher advances before returning an update. Queued block updates are removed with
`pop_front`, and the event watcher is updated before the block is handed to the driver. The driver
then processes transaction housekeeping, state transition, and action enqueueing with fallible
operations.

For errors other than state-transition errors, the run loop logs that it is "retrying after delay",
but the next iteration asks the watcher for a new update. The failed update is not retained.

Concrete failure modes include:

- If transaction housekeeping encounters a transient RPC error while processing a new block, the
  state machine never receives that block. The next watcher item is normally the block's logs,
  which the state machine rejects as out of sequence.
- The resulting state error is classified as unrecoverable and terminates the driver.
- If the state snapshot is committed but transaction enqueueing fails, the action is lost while the
  state has already advanced.
- If enqueueing succeeded and only submission failed, the durable transaction is safe, but the
  driver does not distinguish that case from an enqueue failure.

The state machine also cannot survive its own storage failures. `StateMachine::handle_update`
takes the in-memory state (`mem::take` in [`core/state/mod.rs`](../crates/core/src/state/mod.rs))
and only restores it on success; an error from `snapshots.commit` or `snapshots.reorg` -
including a transient SQLite failure - leaves the machine permanently `Poisoned`. The driver
classifies every state error as unrecoverable, so a single failed snapshot commit terminates the
validator (with a zero exit status) instead of being retried.

The documented guarantee that an update is processed to completion is therefore not true under
internal failures. An unrecoverable driver error also returns through `main` as success, producing a
zero exit status.

Required remediation:

- Introduce an acknowledgement or durable-outbox model.
- Persist the state transition and encoded logical actions atomically.
- Drain the durable outbox independently from state advancement.
- Only acknowledge watcher progress after the durable transition has committed.
- Restore the in-memory state and retry when a snapshot commit fails transiently, instead of
  poisoning the state machine.
- Return a nonzero process status for unrecoverable state errors.
- Add fault-injection tests for transaction housekeeping, state persistence, action enqueueing, and
  transaction submission.

### 4. Secret and nonce pruning is not reorg-safe

DKG secrets and retired group nonce trees are deleted immediately during finalization, failure,
restart, and rollover. Effects run before the corresponding state snapshot is committed, and the
secret store is not rolled back with state snapshots.

The clearest failure is an epoch-boundary reorg:

1. The validator rolls over and immediately deletes the old group nonce trees.
2. A valid reorg restores a snapshot in which the old epoch is active.
3. The restored state still contains the old key share, but its preprocessing trees are gone.
4. The validator cannot reveal nonce commitments already registered on chain for that group.

If the full validator set behaves this way, quorum can become unavailable after the reorg.

There is a related DKG risk if finalization and pruning happen within the reorg window. A rollback
to before setup can force secret resampling while a previously allocated commitment transaction is
still available for resubmission, resulting in a commitment and local-secret mismatch.

Commit `7126078` originally designed pruning as reorg-aware work queued with its originating block
and executed only after the block became safe. Commit `424d678` removed that queue and replaced it
with immediate effects.

The TypeScript implementation also unregisters retired FROST groups immediately, so this is partly
an inherited protocol risk rather than a pure parity regression.

Required remediation:

- Restore the snapshotted pruning queue.
- Execute DKG-secret and nonce-tree deletion only after the originating block is at or below the
  safe boundary.
- Test reorgs across DKG finalization, DKG failure, epoch rollover, and old-group retirement.

## Medium-severity findings

### 5. Synthetic restart replay duplicates non-idempotent actions

Every Rust restart deliberately replays the most recent `max_reorg_depth` blocks through a
synthetic reorg. Replayed state transitions reproduce actions, but transaction storage assigns only
an auto-increment row ID and blindly inserts every encoded request.

This can create a second logical action even when the original canonical transaction was already
executed. `FROSTCoordinator.sign` is particularly sensitive because each call increments the group
sequence and opens a new signing ceremony; it does not reject duplicate messages.

Replay duplication is partially bounded by the block deadlines attached to queued actions: a
replayed action whose `expires_at` has already passed is dropped before submission. It is not
bounded for `Sign`, whose deadline (six blocks by default) exceeds the replay depth (five blocks
by default), nor for actions queued without a deadline.

Repeated restarts can therefore:

- Open orphan signing ceremonies.
- Consume preprocessing sequence numbers and nonce stock.
- Generate unnecessary fallback and top-up work.
- Spend validator gas on duplicate calls or expected reverts.

Required remediation:

- Derive a stable action ID from the originating event coordinates and action kind.
- Upsert or reconcile actions by logical identity.
- Reuse the existing logical action during reorg recovery instead of appending another row.
- Add restart tests around every state-generated action, especially `Sign`.

### 6. Restarting during a historical warp can fail deterministically

During a historical warp, state persistence prunes intermediate snapshots to the final block of
each processed page. The state-machine test explicitly confirms that only the latest snapshot
survives.

If the process restarts after a sufficiently advanced warp page:

1. The latest committed snapshot is block `S`.
2. Startup generates a synthetic uncle around `S - max_reorg_depth`.
3. Reorg restoration requires the uncle's parent snapshot.
4. That snapshot was deleted during warp pruning.
5. Snapshot storage returns `MissingSnapshot`, and the driver exits.

This applies both to a restart in the middle of a long warp and to the window between completing a
warp and building a full set of recent snapshots.

Required remediation:

- Retain at least `max_reorg_depth + 1` snapshots while warping, or
- Persist a warp checkpoint and resume mode that does not synthesize a reorg until a normal reorg
  window has been established.
- Add process-restart tests after every warp page and during recent-block catch-up.

### 7. Event type is not validated against its emitter address

Rust watches the consensus contract, coordinator, and configured oracles in one address filter. The
union event decoder classifies logs only by topics and data, returning the first ABI interface that
decodes. Coordinator and consensus transition handlers then ignore the log address; only
`OracleResult` uses it.

A malicious or upgraded configured oracle can therefore emit a byte-compatible `Sign`,
`Preprocess`, `EpochStaged`, or other coordinator or consensus event. The validator will process the
log with authority greater than an oracle is intended to possess.

The TypeScript watcher has the same trust-boundary flaw because it also uses one combined address
and event filter.

Required remediation:

- Dispatch using both event type and expected emitter address.
- Prefer separate per-contract decoders or explicit address-to-interface routing.
- Add tests in which each watched contract emits a topic belonging to another watched contract.

### 8. Default configuration is not mixed-network parity-safe

The implementations use different implicit defaults:

| Setting          |  Rust | TypeScript |
| ---------------- | ----: | ---------: |
| Blocks per epoch | 1,440 |     17,280 |
| Keygen timeout   |   120 |        120 |
| Signing timeout  |     6 |        120 |
| Oracle timeout   |    12 |        120 |

Some Rust values match the current beta documentation, so parts of this difference appear
intentional. Silent defaults remain dangerous in a mixed Rust and TypeScript validator set,
especially when the oracle timeout changes when validators approve, decline, or submit fallback
actions.

Staker behavior also differs:

- TypeScript defaults the staker to the validator account.
- Rust skips staker reconciliation entirely when `staker` is absent.

Required remediation:

- Make consensus-affecting timing explicit and required in production configuration.
- Publish a parity table and checked-in Rust TOML example.
- Preserve the TypeScript staker default or document and validate the semantic change.
- Add mixed-validator tests with default and explicitly configured timeout values.

### 9. Production lifecycle and observability are incomplete

The Rust service has substantially less lifecycle and health instrumentation than the TypeScript
service:

- Rust waits only for `Ctrl-C`; TypeScript handles both `SIGINT` and `SIGTERM`.
- Unrecoverable state errors terminate the driver but allow `main` to return success.
- A Prometheus endpoint is installed, but validator and core code emit no production counters,
  gauges, or histograms.
- TypeScript exposes block cursor, event index, reorg, transition, RPC, transaction, cleanup, and
  process metrics.
- There is no dedicated Rust validator README, migration runbook, or checked-in example TOML.

Required remediation:

- Handle `SIGTERM` and complete in-flight durable work before shutdown.
- Propagate unrecoverable failures to a nonzero process exit.
- Add progress, reorg, transition, effect, transaction-queue, nonce-stock, DKG, and signing metrics.
- Document configuration, startup state, database backup, migration, and rollback procedures.

## Low-severity finding

### 10. Participant safety threshold can overflow for very large groups

`min_participants` computes `count * 2 / 3 + 1` using `u16`. For a participant count of at least
32,768, debug builds panic and release builds wrap, potentially violating the required
greater-than-two-thirds threshold.

Such a validator set is currently impractical, but the threshold is a security invariant and
should not depend on an implicit deployment-size assumption.

Required remediation:

- Use a wider intermediate or an overflow-safe equivalent such as `count - count / 3`.
- Add an explicit supported participant-count bound to configuration validation.
- Test the boundary values of the supported range.

## Improvements over the TypeScript implementation

The Rust implementation contains important improvements that should be preserved while addressing
the findings:

- It uses the maintained Zcash FROST implementation and includes cross-language parity vectors.
- It validates generated and public key material more rigorously before accepting a group.
- Duplicate transaction and oracle proposals do not reset an ongoing signing state, preventing a
  proposal-based liveness attack.
- Signature-share participation is tracked per selection root, avoiding attribution across
  competing signer selections.
- Nonce top-up considers retained groups and the global sequence behavior rather than only the
  current active group.
- The transaction queue persists nonce allocation, fee bumps, execution status, and reorg markers.
  This is a meaningful architectural improvement once retry and deduplication are corrected.
- TOML parsing, nonzero timeout types, explicit epoch types, unknown-field rejection, and resolving
  the coordinator from the consensus contract improve maintainability.
- Rust correctly rejects delegate-call subtransactions inside `MultiSendCallOnly`. TypeScript
  supplies its delegate-call policy to both ordinary MultiSend and CallOnly variants. Rust is safer,
  but a mixed validator set can disagree on the same proposal; the TypeScript policy should be
  corrected or the difference explicitly versioned.

## Test coverage assessment

The available Rust checks passed during review:

- `cargo fmt --all -- --check`
- `cargo clippy --package validator --all-targets -- -D warnings`
- `cargo test --package validator` — 51 tests passed
- `cargo clippy --package safenet-core --all-targets -- -D warnings`
- `cargo test --package safenet-core` — 79 tests passed
- `cargo test --workspace` — 146 tests passed across core, sentinel, and validator

TypeScript tests and the process-level integration scripts could not be run in the review
environment because Node.js and NPM were unavailable. The all-Rust integration suite is driven by
the TypeScript test harness and was therefore also unavailable. A coverage run was attempted but
exhausted the available disk space while compiling; its generated artifacts were removed.

The test-shape difference remains material:

- The Rust validator has 51 tests, but only one direct unit test in the state-transition modules.
- The TypeScript validator has 67 test files and approximately 506 `it` or `test` cases.
- The shared process integration file contains four broad cases: keygen timeout, keygen abort,
  keygen and signing, and oracle signing.

High-value missing scenarios include:

- Transient failure of every effect.
- Driver failure before and after state persistence and action enqueueing.
- Cold start after genesis and migration from an active TypeScript deployment.
- Restart during action submission and deterministic action deduplication.
- Restart after each historical warp page.
- Reorg across DKG setup, finalization, failure, and epoch cleanup.
- Spoofed events from the wrong watched emitter.
- Delayed, rejected, or missing oracle responses under mixed timeout configuration.

### Required differential consensus test matrix

The existing mixed test demonstrates cryptographic interoperability on a successful path. It does
not establish state-machine compatibility. A release-grade differential suite should feed the same
ordered block and event trace to one Rust validator model and one TypeScript validator model, then
compare their normalized consensus state and intended actions after every transition.

At minimum, the normalized comparison must include:

- Active and target epoch.
- Active, staged, and in-progress group IDs and participant sets.
- Packet hash and expected signing group.
- Current signature ID, phase, signer selection root, and contributing participants.
- Responsible participant or “everyone” state.
- The logical action kind, packet, group, and retry generation.

The suite should cover the following traces:

1. Submit duplicate plain and oracle proposals before the first ceremony completes. Both
   implementations must retain the same signature ID and issue no second oracle request.
2. Call `FROSTCoordinator.sign` directly with a valid packet but the wrong finalized group while
   the packet is waiting for a request. Both implementations must reject the event as unrelated.
3. Exercise invalid DKG commitment encodings, invalid encrypted-share counts, an invalid local
   encrypted share, and a public verification share that does not match its commitments. Every
   observer must count the same contributors and derive the same replacement group. Repeat with an
   off-curve DKG or nonce point before valid logs in the same live block and historical warp page;
   the valid suffix must still be processed.
4. Time out every keygen and signing phase with a named responsible participant and with
   responsibility assigned to everyone. Assert the same action sender set, exclusion set, retry
   generation, and cleanup block. Hold an action in each queue across its block deadline and assert
   that neither implementation later submits an obsolete generation.
5. Submit `SignShared` events under two competing selection roots, including a share under a root
   that cannot reach threshold. Both implementations must select and retry from the same root.
6. Complete the signature but make the callback fail for rollover, plain transaction, and oracle
   packets. Assert identical direct-attestation fallback and cleanup behavior.
7. Introduce a reorg after each consensus-relevant event class: proposal, `Sign`, `Preprocess`,
   DKG commitment/share/confirmation, rollover, signature share, and final attestation. Compare
   state and actions after canonical replay.
8. Finalize genesis exactly one block before an epoch boundary. Both implementations must derive
   the same first regular target epoch and group context.
9. Rotate the active epoch while an old-group ceremony remains live, then force a retry and nonce
   top-up. Both implementations must replenish and reveal nonces for the event's group. Interleave
   another participant's `Preprocess` event before the validator's own event and assert that it does
   not clear or duplicate the validator's pending tree.
10. Deliver an oracle result immediately before, at, and immediately after the configured
    deadline. Assert identical acceptance, decline, timeout, and cleanup decisions.
11. Run a shared Safe-policy corpus through both implementations, including every configured
    MultiSend and `MultiSendCallOnly` address and delegate-call position, plus plain calls
    (operation 0) to every policy-listed address. Approval and decline must match exactly.
12. Start each implementation with omitted, equal, and intentionally unequal timing parameters.
    Production mode must reject omitted or incompatible consensus settings rather than silently
    constructing different state machines.

These assertions should compare consensus state, not only eventual on-chain success. A Rust
majority can hide a TypeScript incompatibility by completing a ceremony that the TypeScript subset
abandoned, and the reverse can hide a Rust liveness problem. Known intentional Rust fixes should
be encoded as the required behavior and backported to TypeScript; they should not be accepted as
permanent “expected differences” in this suite.

## Release recommendation

Before the Rust port is treated as complete or used as a production replacement, require the
following release gates:

1. Define one validator consensus-behavior version. Backport the Rust duplicate-proposal, public
   DKG validation, selection-root attribution, expected-group binding, old-group nonce top-up,
   event-local curve validation, preprocessing ownership, terminal cleanup, oracle fallback,
   CallOnly policy, and operation-routing policy fixes to TypeScript. Reconcile this list against
   the divergences already accepted as project decisions (see the compatibility model above).
2. Align the DKG, signing, timeout-responsibility, reorg, and first-post-genesis transitions. Do not
   deploy mixed Rust and TypeScript signing groups until the differential matrix passes.
3. Remove implicit production timing defaults and validate that blocks-per-epoch and all phase
   deadlines match the network's declared consensus configuration; expire actions by those same
   block deadlines.
4. Implement and test live-network bootstrap and migration behavior.
5. Replace swallowed one-shot effect failures with durable retries.
6. Introduce an atomic state/action outbox and real update acknowledgement, and retry transient
   snapshot-commit failures instead of poisoning the state machine.
7. Restore safe-block-delayed secret and nonce pruning.
8. Add stable logical action identities for restart and reorg deduplication.
9. Fix historical-warp checkpoint recovery.
10. Validate every decoded event against its emitter address.
11. Add production shutdown, failure exit, metrics, and operator documentation.
12. Run the TypeScript, all-Rust, and mixed integration suites in CI, including the full
    differential matrix plus failure, restart, migration, and reorg scenarios.
