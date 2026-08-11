# Plan: Bind `oracleData` into the Oracle Transaction Proposal Attestation

Component: `contracts/` (Solidity: `Consensus`, `ConsensusMessages`, `IConsensus`, `SafenetGuard`, `AttestationTrailer`), `crates/validator` (Rust), `crates/sentinel` (Rust), `explorer/` (React/TypeScript), `examples/`, guard docs.

---

## Overview

Today `oracleData` is the arbitrary, oracle-specific blob a proposer passes to `Consensus.proposeOracleTransaction(oracle, oracleData, transaction)` (`contracts/src/Consensus.sol:347`). It is forwarded to the oracle as request input (`IOracle(oracle).postRequest(message, msg.sender, oracleData)`, `Consensus.sol:359`) and is **deliberately excluded from the FROST-signed message** - the message the validator group signs is `oracleTransactionProposal(epoch, oracle, safeTxHash)` (`Consensus.sol:353`), and both the `IConsensus` and `IOracle` NatSpec say so verbatim ("not part of the signed message hash", `contracts/src/interfaces/IConsensus.sol:304`; "not part of the signed message hash", `contracts/src/interfaces/IOracle.sol:30`). The `SafenetGuard` attestation trailer never carries it, and the guard never sees it.

The team wants `oracleData` to become a first-class, authenticated part of the attestation, so a future guard (or any verifier) can rely on it. That is the **bind** design (Option 2 from the solution-space assessment): add `oracleData` as a `bytes` member of the signed `OracleTransactionProposal` message, carry the raw bytes in the guard's attestation trailer and emit them in the `OracleTransactionProposed` event, and reconstruct-and-verify on-chain. It is authenticated end-to-end from day one and is empty today (`keccak256("")` in the struct hash), so it carries no meaning yet but needs no further wire-format change to start carrying one.

Because the signed message hash equals the oracle `requestId` (`Consensus.sol:353-359`, and `oracleTransactionProposal`'s own doc: "used as the oracle requestId", `contracts/src/libraries/ConsensusMessages.sol:118`), binding `oracleData` is **not** a guard-local change. Every producer and consumer of that hash has to agree at once:

1. **On-chain signer input** - `ConsensusMessages.oracleTransactionProposal` (the message the FROST coordinator signs) and every function that rebuilds it: `Consensus.proposeOracleTransaction` (which computes it and keys the oracle request/`$attestations` by it), `attestOracleTransaction` and `getOracleTransactionAttestationByHash` (which recompute it, lines 398/413), and the `attestOracleTransaction` branch of `onSignCompleted` (which decodes the reconstruction context, 455-458).
2. **The event validators read to reconstruct the message** - `OracleTransactionProposed` (`IConsensus.sol:110-117`) carries `epoch/oracle/safeTxHash` but **not** `oracleData`, so off-chain reconstructors cannot form the new message until the event emits it.
3. **The Rust validator's hashing mirror** - `crates/validator/src/consensus/hashing.rs:43,127,144` (`OracleTransactionProposal` struct, `oracle_transaction_proposal_hash`, `oracle_transaction_packet_hash`) plus the event reconstruction at `crates/validator/src/state/transactions.rs:110,135,175`.
4. **The Rust sentinel's hashing mirror** - `crates/sentinel/src/hashing.rs:82` (`oracle_tx_proposal_hash`, "the EIP-712 requestId (the proposal hash)") with its Solidity-parity test at `:146`, plus `crates/sentinel/src/bindings.rs:89,97` and `crates/sentinel/src/service.rs:115,626,781`.
5. **The explorer's client-side mirror** - `explorer/src/lib/oracle/hashing.ts:11-28` ("Mirrors `ConsensusMessages.oracleTransactionProposal`") and the `OracleTransactionProposed` ABI fragment in `explorer/src/lib/consensus/abi.ts`.
6. **The guard consumer** - `AttestationTrailer` (the codec, `contracts/src/libraries/AttestationTrailer.sol`) and `SafenetGuard.checkTransaction` (`contracts/src/guard/SafenetGuard.sol:200-230`), which must carry the raw `oracleData` in the trailer and rebuild the message.
7. **The reference example** - `examples/attest-safe-tx.ts` builds the trailer and the signed message by hand (`:54,183,271`).

None of this is deployed (the guard is unreleased; the protocol runs on devnet), so there is no runtime backward-compatibility window to preserve - only per-PR compile/test consistency and cross-language integration-test consistency to keep green.

---

## Architecture Decision

**Bind `oracleData` as a `bytes` member of the signed message.** The message gains one field: `OracleTransactionProposal(uint64 epoch,address oracle,bytes32 safeTxHash,bytes oracleData)`. Under EIP-712 a dynamic `bytes` member is encoded as `keccak256(oracleData)` in the struct hash, so `oracleData` is hashed exactly once and the struct-hash preimage is fixed-width either way - declaring it as raw `bytes` rather than a pre-hashed `bytes32` costs nothing on-chain (same single hash, same static struct) and keeps the signed message self-describing: EIP-712 tooling and any reader see the actual `oracleData`, not an opaque hash. The only real consequence of raw-vs-hash is what the event and the reconstruction paths carry, not the digest or its cost.

**Emit the full `oracleData` in `OracleTransactionProposed` (and `OracleTransactionAttested`).** Off-chain reconstructors form the message from the event, and every function that rebuilds the message hash (the validator, the sentinel, `attestOracleTransaction`, `getOracleTransactionAttestationByHash`, `onSignCompleted`) needs the raw `oracleData` since the EIP-712 encoding hashes it. Emitting the whole blob rather than a hash is therefore what the reconstruction paths require, and it lets observers read the actual data instead of a hash they cannot invert. The event already carries the full `SafeTransaction.T`, so a variable-length `oracleData` field fits its existing shape.

**Carry raw `oracleData` in the guard trailer as a fifth, variable-length payload field.** The payload becomes `abi.encode(uint64 epoch, address oracle, Secp256k1.Point groupKey, FROST.Signature signature, bytes oracleData)`. The `SignatureExtension` transport is already length-prefixed and variable-length (`contracts/src/libraries/SignatureExtension.sol:14`), so only `AttestationTrailer`'s fixed-224-byte assertion (`AttestationTrailer.sol:75`) blocks this. The guard decodes `oracleData` and passes the raw bytes into `ConsensusMessages.oracleTransactionProposal` (which hashes the member per EIP-712), rebuilding the exact message the network signed - so `oracleData` is authenticated by the same FROST signature that already authenticates the rest.

**Land the coupled consensus loop (Solidity core + Rust validator + Rust sentinel) as one PR.** Unlike a removal, this is a shared wire-format change: the moment Solidity signs/keys by the new hash but the validator or sentinel still computes the old one, the cross-language integration jobs (`sentinel-integration`, and `validator-integration` if wired) go red on the round-trip. Those jobs run on `pull_request`, so a Solidity-only PR would ship red CI. The three surfaces that participate in the propose to attest round-trip therefore move together. The guard, explorer, examples, and docs are *separate* consumers not exercised by those integration scripts and get their own PRs.

**Redefine `v1` of the attestation trailer rather than bumping to `v2`.** The guard is unreleased and has no on-chain consumers, so `keccak256("SafenetGuard.AttestationTrailer.v1")` can be repointed at the new payload schema, exactly as the oracle-attestation migration itself repointed the v1 payload. This is the lowest-churn option (examples/docs already say `v1`), and the team confirmed keeping `v1` since the earlier version was never released.

**`oracleData` is empty today.** Every current proposer passes `""`, so the committed member is `keccak256("")` and nothing changes semantically. The field exists, is authenticated, and can start carrying meaning later with zero wire-format change.

### Alternatives Considered

- **Option 1: carry `oracleData` in the trailer unsigned (no message change).** Rejected per the team's decision: it makes the field a malleable, unauthenticated hint, and any future use that must *trust* it would still need this bind change later - a second migration and a second protocol-wide re-coordination. Doing the bind now, while everything is unreleased and empty, is strictly cheaper.
- **Commit a pre-hashed `bytes32 oracleDataHash` in the message and event instead of raw `bytes`.** Rejected (this was the plan's first draft, corrected in review): it is not cheaper (EIP-712 hashes the `bytes` member exactly once either way, and the struct-hash preimage is fixed-width regardless), and it makes the signed message and event opaque (a hash instead of the data) while forcing observers to obtain the raw `oracleData` out-of-band to interpret it. Raw `bytes` is the idiomatic EIP-712 shape and lets any reader see the actual `oracleData`; the only cost is a larger event/calldata when `oracleData` is non-empty, which is bounded and is the data consumers want anyway.
- **Carry `oracleData` as a second, separate signature extension rather than a new field in the existing payload.** Rejected: `SignatureExtension` is explicitly single-extension, no nesting (`SignatureExtension.sol:30`); a second envelope would need a new transport format for no benefit over one extra `abi.encode` field.
- **Split the Solidity/validator/sentinel message change into three independent PRs (as the legacy-removal epic did).** Rejected: those removals were each independently valid at runtime; this is a matched wire-format pair whose halves are wrong in isolation, and the integration jobs prove it on every PR. Compiling independently (hand-written `sol!`/ABI mirrors) is not the same as integration-testing independently.
- **Keep `requestId` decoupled from the signed message (bind only the guard-side message).** Not possible: `Consensus` uses the one `message` value both as the coordinator's signing input and as the oracle `requestId`/`$attestations` key. Binding the signed message necessarily rekeys the oracle request. This is a real semantic change (see Resolved Decisions), not an incidental one.

---

## Tech Specs

### Phase 1 - Bind `oracleData` into the consensus message across the coupled loop (Solidity + Rust validator + Rust sentinel)

One PR. Adds this epic file (transient; removed in the final phase). Self-consistent: every language's own unit tests and the cross-language integration scripts stay green because all three hashers change together.

**Solidity core**

- `contracts/src/libraries/ConsensusMessages.sol`:
  - Change `ORACLE_TRANSACTION_PROPOSAL_TYPEHASH` (35-36) to `keccak256("OracleTransactionProposal(uint64 epoch,address oracle,bytes32 safeTxHash,bytes oracleData)")`; recompute the precomputed hex literal and update the `@custom:precomputed` comment (33).
  - Extend `oracleTransactionProposal` (120-136) with a trailing `bytes memory oracleData` parameter (memory so both the `Consensus` calldata caller and the guard's memory caller can pass it); compute `keccak256(oracleData)` and store it as the fifth struct word (`mstore(add(ptr, 0x80), <hash>)`), widening the struct-hash `keccak256(ptr, 0x80)` to `keccak256(ptr, 0xa0)` (the `0x1901`/domain-separator wrapping is unchanged). Update the NatSpec.
- `contracts/src/Consensus.sol` `proposeOracleTransaction` (347-360): pass `oracleData` (already `calldata`) into `oracleTransactionProposal(...)` (353); add the full `oracleData` to the `OracleTransactionProposed` emit (355-357). `message` is still both the coordinator signing input (358) and the `postRequest` `requestId` (359) - now binding `oracleData`.
- `contracts/src/Consensus.sol` - the other two callers of `oracleTransactionProposal` and the sign callback, all of which rebuild the same `message` and would otherwise fail to compile once the fifth field is required:
  - `attestOracleTransaction` (389-403): add a trailing `bytes calldata oracleData` parameter, thread it into `oracleTransactionProposal(epoch, oracle, safeTxHash, oracleData)` (398); the `$attestations[message]` key follows automatically. Add the full `oracleData` to the `OracleTransactionAttested` emit (402) so off-chain consumers can rebuild/key the attestation the same way.
  - `getOracleTransactionAttestationByHash` (408-415): add the same `bytes calldata oracleData` parameter and pass it at 413. This is a public getter signature change, so every off-chain caller (explorer, `examples/attest-safe-tx.ts`, any script) must supply it - handled in their own phases.
  - `onSignCompleted` (444-462): in the `attestOracleTransaction` branch, widen the decoded context tuple (457) to `(uint64, address, uint256, address, bytes32, bytes)` - the added field is the raw `oracleData` - and pass it to `attestOracleTransaction` (458). The context itself is supplied by the signature-completing party via `FROSTCoordinator.signShareWithCallback` (`contracts/src/FROSTCoordinator.sol:639-648`), i.e. it is encoded *off-chain* by the Rust validator, so on-chain only the decode changes here; the encoder change lives in the validator sub-phase below.
- `contracts/src/interfaces/IConsensus.sol`: add `bytes oracleData` to the `OracleTransactionProposed` event (110-117) and the `OracleTransactionAttested` event, each with a matching `@param`; add the same trailing `bytes oracleData` parameter (and `@param`) to the `attestOracleTransaction` and `getOracleTransactionAttestationByHash` declarations; flip the `proposeOracleTransaction` `@param oracleData` NatSpec (304) from "not part of the signed message hash" to "bound into the signed message hash (as the `bytes oracleData` EIP-712 member)". Update `contracts/src/interfaces/IOracle.sol:30`'s parallel "not part of the signed message hash" note.
- Tests: `contracts/test/libraries/ConsensusMessages.t.sol` - update/extend the `oracleTransactionProposal` vector to include `oracleData` and assert the new precomputed typehash. `contracts/test/Consensus.t.sol` - assert the emitted `oracleData`, that `message`/`requestId` now depends on `oracleData`, and that two proposals for the same tx with different `oracleData` no longer collide on `AlreadyAttested` (see Resolved Decisions).

**Rust validator mirror**

- `crates/validator/src/bindings.rs:148`: add `bytes oracleData` to the `OracleTransactionProposed` `sol!` event.
- `crates/validator/src/consensus/hashing.rs`: add `oracle_data: Bytes` to `struct OracleTransactionProposal` (43); add the parameter to `oracle_transaction_proposal_hash` (127) and `oracle_transaction_packet_hash` (144), hashing it as the EIP-712 `bytes` member; update the `sample_oracle_transaction_packet_hash` expected value (216) to the new Solidity hash.
- `crates/validator/src/state/transactions.rs`: thread `event.oracleData` into the hash reconstruction at 135 and 175 (and the `handle_oracle_transaction_proposed` signature at 110 if the field must reach it).
- `crates/validator/src/service/action.rs` and `crates/validator/src/state/sign.rs`: the `AttestOracleTransaction` action encodes the `onSignCompleted` callback context that the extended on-chain decode reads, so add `oracleData` (raw bytes) to that context, and to wherever the validator reconstructs the oracle-proposal message it attests to.

**Rust sentinel mirror**

- `crates/sentinel/src/bindings.rs`: add `oracleData` to `struct OracleTransactionProposal` (89) and the `OracleTransactionProposed` event (97).
- `crates/sentinel/src/hashing.rs`: add the `oracle_data` parameter to `oracle_tx_proposal_hash` (82), hashing it as the EIP-712 `bytes` member, and update the Solidity-parity test `oracle_tx_proposal_hash_parity` (146).
- `crates/sentinel/src/service.rs`: thread the new field through the `OracleTransactionProposed` handling at 115, 626, 781.

### Phase 2 - Guard: carry and verify `oracleData`

Stacked on Phase 1 (the guard tests build the expected message via the new `ConsensusMessages.oracleTransactionProposal` signature).

- `contracts/src/libraries/AttestationTrailer.sol`:
  - Change the payload schema to `abi.encode(uint64 epoch, address oracle, Secp256k1.Point groupKey, FROST.Signature signature, bytes oracleData)`. The head is now 256 bytes (224 static + one 32-byte offset word) plus a 32-byte length word and padded data, so the minimum payload (empty `oracleData`) is 288 bytes, not a fixed 224.
  - Replace the exact `require(payloadData.length == _PAYLOAD_LENGTH, MalformedAttestationTrailer())` (75) with a minimum-length guard `require(payloadData.length >= 288, MalformedAttestationTrailer())` (288 = the empty-`oracleData` minimum: 256-byte head + 32-byte length word) followed by `abi.decode(payloadData, (uint64, address, Secp256k1.Point, FROST.Signature, bytes))`, whose own offset/length validation over the `SignatureExtension`-bounded slice rejects the rest. The guard keeps its own `MalformedAttestationTrailer` error for the recognised-but-too-short case rather than surfacing a bare `abi.decode` panic. Add `bytes memory oracleData` to `decode`'s return tuple and NatSpec; turn `_PAYLOAD_LENGTH` (33) into the `288` minimum and update the header doc (12-13, 31).
  - Update the `TYPE_HASH` payload description; keep `keccak256("SafenetGuard.AttestationTrailer.v1")` (the earlier version was never released, so v1 is repointed rather than bumped).
- `contracts/src/guard/SafenetGuard.sol` `checkTransaction` (200-230): capture `bytes memory oracleData` from `decode` and pass the raw bytes into `ConsensusMessages.oracleTransactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, oracle, safeTxHash, oracleData)` (227-228), which hashes the member per EIP-712, before `FROST.verify` (229). Update the comment (223-226) to note `oracleData` is now authenticated by the same signature.
- Tests: `contracts/test/libraries/AttestationTrailer.t.sol` - round-trip with empty and non-empty `oracleData`, plus a malformed-payload revert. `contracts/test/SafenetGuard.t.sol` - extend `_buildInlineAttestation` (175-186) to append `oracleData` to the payload and fold it into the message it signs; add a success case with non-empty `oracleData` and a tamper case (trailer `oracleData` altered so the rebuilt message breaks `FROST.verify`).

### Phase 3 - Guard documentation

Stacked on Phase 2.

- `contracts/src/guard/DESIGN.md:18-20`: update the trailer bullet to the five-field, variable-length payload; update the "attested message" bullet to `epoch + oracle + nonce-bound Safe tx hash + oracleData`; note `oracleData` is now authenticated end-to-end.
- `contracts/src/guard/README.md:21,24,27,29,41,47,48,55`: update the payload layout diagram (`abi.encode(..., bytes oracleData)`), the `224`/`288`-byte figures (payload is now `>= 288`; total overhead `>= 352` for empty `oracleData`, growing with its length), the `checkTransaction`-outcomes paragraph, and the `OracleTransactionProposal` message description. The `TYPE_HASH` stays `v1`, so no version wording changes.

### Phase 4 - Explorer client-side mirror

Depends on Phase 1 (new event field and message definition). Independent of the guard phases; hand-written ABI/typed-data mirrors, so no compile coupling.

- `explorer/src/lib/oracle/hashing.ts:11-28`: add the `bytes oracleData` member to the `OracleTransactionProposal` typed-data types and to the function's inputs; keep it a faithful mirror of the updated `ConsensusMessages.oracleTransactionProposal`.
- `explorer/src/lib/consensus/abi.ts`: add `oracleData` to the `OracleTransactionProposed` event fragment (and any event-selector list that must stay in sync).
- `explorer/src/lib/coordinator/signing.ts:121` (and callers of the oracle hashing path): thread the raw `oracleData` where the oracle proposal message is reconstructed for signing/verification. (The non-oracle `safeTxProposalHash`/`packets.ts` path is unrelated and untouched.)
- Update the affected explorer tests; manually verify in a browser (repo convention for frontend changes) that an oracle proposal's voting/verification UI still loads.

### Phase 5 - Reference example

Depends on Phase 1 (message) and Phase 2 (trailer payload). `examples/attest-safe-tx.ts`:

- Append `oracleData` (empty by default) to the `abi.encode` payload (241-271) as the new fifth field, and include it in the reconstructed `OracleTransactionProposal` typed-data message it signs (183 and around 54), where EIP-712 hashes the member.
- Keep `TYPE_HASH` at `v1` (54); the schema is repointed, not versioned.
- Reflect the change in the file's header walkthrough comment (4-12).

### Phase 6 - Remove this plan

Delete `epics/2026_08_11_bind_oracle_data.md` once Phases 1-5 are merged.

---

## Implementation Phases

| Phase | Summary | Depends on | Own PR |
|---|---|---|---|
| 1 | Bind `oracleData` (as a `bytes` field) into `OracleTransactionProposal`: Solidity core (`ConsensusMessages`/`Consensus`/`IConsensus` + event field + tests) **and** the Rust validator and sentinel hashing mirrors, together (integration-test-coupled). Adds this epic. | - | ✅ |
| 2 | Guard: carry raw `oracleData` in the trailer (`AttestationTrailer` five-field payload) and rebuild+verify the message in `SafenetGuard.checkTransaction`; guard forge tests | 1 | ✅ |
| 3 | Guard docs: `DESIGN.md`/`README.md` payload layout, overhead figures, message and trust-model wording | 2 | ✅ |
| 4 | Explorer client-side mirror: `oracle/hashing.ts` typed data, `consensus/abi.ts` event, `coordinator/signing.ts`, tests | 1 | ✅ |
| 5 | Reference example: `examples/attest-safe-tx.ts` payload + message | 1, 2 | ✅ |
| 6 | Remove this plan | 1-5 | ✅ |

Phase 1 is the single coupled unit. Phases 2 and 4 depend only on Phase 1 and can proceed in parallel (guard vs explorer); Phase 3 stacks on 2; Phase 5 needs both 1 and 2; Phase 6 closes it out.

---

## Resolved Decisions and Assumptions

The team reviewed the questions below; each is now decided.

- **Bind raw `bytes oracleData`, not a `bytes32` hash.** Per review, the signed message uses a `bytes oracleData` member (EIP-712 hashes it once, per spec) and the event emits the full `oracleData`. This is no more expensive than a pre-hashed commitment and keeps the message and event self-describing rather than opaque.
- **`requestId` semantics change - confirmed intended.** Because the signed message equals the oracle `requestId` and the `$attestations` key, binding `oracleData` means the same Safe transaction proposed with different `oracleData` now produces distinct request ids (today they collide on `AlreadyAttested`). The team confirmed this is the intended protocol behaviour (different oracle input = different request).
- **Keep `v1`, do not bump to `v2`.** `keccak256("SafenetGuard.AttestationTrailer.v1")` is repointed at the new payload schema, since the earlier version was never released and has no on-chain consumers, exactly as the oracle-attestation migration repointed v1.
- **Add a minimum-length guard for `MalformedAttestationTrailer`.** Because a variable-length payload has no single valid length, Phase 2 adds `require(payloadData.length >= 288, MalformedAttestationTrailer())` before `abi.decode` (288 = the empty-`oracleData` minimum), so a recognised-but-malformed payload still reverts with the guard's own error rather than a bare `abi.decode` panic.
- **Sentinel stays in Phase 1 as hash-coupled.** `crates/sentinel/src/hashing.rs` has `oracle_tx_proposal_hash` with a Solidity-parity test, so the plan treats the sentinel as hash-coupled and includes it in Phase 1. The exact coupling (recompute vs. read `requestId` from the oracle's own events) is confirmed during Phase 1 implementation; if the sentinel never recomputes, it can be split out then.
- **Keep all three surfaces in Phase 1.** Solidity core, the Rust validator, and the Rust sentinel land together in Phase 1 so the cross-language integration jobs (`sentinel-integration` today, and `validator-integration` if/when wired) stay green.
- **`oracleData` stays empty in all current call sites.** No existing proposer, script, or example passes non-empty `oracleData`; the plan adds only the authenticated capacity to carry one. First real use is out of scope here.
- **`getOracleTransactionAttestationByHash` is a breaking public-interface change.** It gains a required `bytes oracleData` argument, so every caller is updated: the explorer (Phase 4), `examples/attest-safe-tx.ts` (Phase 5), and any `cast`/script usage in docs or `scripts/`. This is intentional and its callers are tracked across phases, not missed.
- **Devnet/integration scripts - verify, low-risk.** `scripts/run_devnet.sh`, `scripts/run_sentinel_integration_test.sh`, and the validator integration script may grep `OracleTransactionProposed`; Phase 1 verifies none parse positionally in a way the new event field breaks. Assumed low-risk (they key on the event name / indexed topics); no dedicated work unless a positional parse turns up.