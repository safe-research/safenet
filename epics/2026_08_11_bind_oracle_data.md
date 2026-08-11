# Plan: Bind `oracleData` into the Oracle Transaction Proposal Attestation

Component: `contracts/` (Solidity: `Consensus`, `ConsensusMessages`, `IConsensus`, `SafenetGuard`, `AttestationTrailer`), `crates/validator` (Rust), `crates/sentinel` (Rust), `explorer/` (React/TypeScript), `examples/`, guard docs.

---

## Overview

Today `oracleData` is the arbitrary, oracle-specific blob a proposer passes to `Consensus.proposeOracleTransaction(oracle, oracleData, transaction)` (`contracts/src/Consensus.sol:347`). It is forwarded to the oracle as request input (`IOracle(oracle).postRequest(message, msg.sender, oracleData)`, `Consensus.sol:359`) and is **deliberately excluded from the FROST-signed message** - the message the validator group signs is `oracleTransactionProposal(epoch, oracle, safeTxHash)` (`Consensus.sol:353`), and both the `IConsensus` and `IOracle` NatSpec say so verbatim ("not part of the signed message hash", `contracts/src/interfaces/IConsensus.sol:304`; "not part of the signed message hash", `contracts/src/interfaces/IOracle.sol:30`). The `SafenetGuard` attestation trailer never carries it, and the guard never sees it.

The team wants `oracleData` to become a first-class, authenticated part of the attestation, so a future guard (or any verifier) can rely on it. That is the **bind** design (Option 2 from the solution-space assessment): add a commitment to `oracleData` to the signed `OracleTransactionProposal` message, carry the raw bytes in the guard's attestation trailer, and reconstruct-and-verify the commitment on-chain. It is authenticated end-to-end from day one and is empty today (`keccak256("")`), so it carries no meaning yet but needs no further wire-format change to start carrying one.

Because the signed message hash equals the oracle `requestId` (`Consensus.sol:353-359`, and `oracleTransactionProposal`'s own doc: "used as the oracle requestId", `contracts/src/libraries/ConsensusMessages.sol:118`), binding `oracleData` is **not** a guard-local change. Every producer and consumer of that hash has to agree at once:

1. **On-chain signer input** - `ConsensusMessages.oracleTransactionProposal` (the message the FROST coordinator signs) and `Consensus.proposeOracleTransaction` (which computes it and keys the oracle request/`$attestations` by it).
2. **The event validators read to reconstruct the message** - `OracleTransactionProposed` (`IConsensus.sol:110-117`) carries `epoch/oracle/safeTxHash` but **not** `oracleData` or its hash, so off-chain reconstructors cannot form the new message without a new event field.
3. **The Rust validator's hashing mirror** - `crates/validator/src/consensus/hashing.rs:43,127,144` (`OracleTransactionProposal` struct, `oracle_transaction_proposal_hash`, `oracle_transaction_packet_hash`) plus the event reconstruction at `crates/validator/src/state/transactions.rs:110,135,175`.
4. **The Rust sentinel's hashing mirror** - `crates/sentinel/src/hashing.rs:82` (`oracle_tx_proposal_hash`, "the EIP-712 requestId (the proposal hash)") with its Solidity-parity test at `:146`, plus `crates/sentinel/src/bindings.rs:89,97` and `crates/sentinel/src/service.rs:115,626,781`.
5. **The explorer's client-side mirror** - `explorer/src/lib/oracle/hashing.ts:11-28` ("Mirrors `ConsensusMessages.oracleTransactionProposal`") and the `OracleTransactionProposed` ABI fragment in `explorer/src/lib/consensus/abi.ts`.
6. **The guard consumer** - `AttestationTrailer` (the codec, `contracts/src/libraries/AttestationTrailer.sol`) and `SafenetGuard.checkTransaction` (`contracts/src/guard/SafenetGuard.sol:200-230`), which must carry the raw `oracleData` in the trailer and rebuild the commitment.
7. **The reference example** - `examples/attest-safe-tx.ts` builds the trailer and the signed message by hand (`:54,183,271`).

None of this is deployed (the guard is unreleased; the protocol runs on devnet), so there is no runtime backward-compatibility window to preserve - only per-PR compile/test consistency and cross-language integration-test consistency to keep green.

---

## Architecture Decision

**Bind a `keccak256(oracleData)` commitment, not the raw bytes, into the signed message.** The message gains one field: `OracleTransactionProposal(uint64 epoch,address oracle,bytes32 safeTxHash,bytes32 oracleDataHash)`. A fixed-width commitment keeps the EIP-712 struct static and cheap, keeps the event field 32 bytes, and is all any hash-reconstructing consumer (validator, sentinel, explorer) needs. The raw `oracleData` bytes are only needed by parties that must *recompute* the commitment (the proposer already has them; the oracle already receives them via `postRequest`; the guard receives them in the trailer). So the raw bytes travel in exactly one new place - the guard trailer - and everywhere else carries the 32-byte hash.

**Emit `oracleDataHash` in `OracleTransactionProposed`.** Off-chain reconstructors form the message from the event; without the commitment in the event they cannot. Emitting the hash (not the raw blob) is sufficient and cheap, and the raw blob is already available to the oracle out-of-band via `postRequest`.

**Carry raw `oracleData` in the guard trailer as a fifth, variable-length payload field.** The payload becomes `abi.encode(uint64 epoch, address oracle, Secp256k1.Point groupKey, FROST.Signature signature, bytes oracleData)`. The `SignatureExtension` transport is already length-prefixed and variable-length (`contracts/src/libraries/SignatureExtension.sol:14`), so only `AttestationTrailer`'s fixed-224-byte assertion (`AttestationTrailer.sol:75`) blocks this. The guard decodes `oracleData`, computes `keccak256(oracleData)`, and rebuilds the exact message the network signed - so `oracleData` is authenticated by the same FROST signature that already authenticates the rest.

**Land the coupled consensus loop (Solidity core + Rust validator + Rust sentinel) as one PR.** Unlike a removal, this is a shared wire-format change: the moment Solidity signs/keys by the new hash but the validator or sentinel still computes the old one, the cross-language integration jobs (`sentinel-integration`, and `validator-integration` if wired) go red on the round-trip. Those jobs run on `pull_request`, so a Solidity-only PR would ship red CI. The three surfaces that participate in the propose to attest round-trip therefore move together. The guard, explorer, examples, and docs are *separate* consumers not exercised by those integration scripts and get their own PRs.

**Redefine `v1` of the attestation trailer rather than bumping to `v2`.** The guard is unreleased and has no on-chain consumers, so `keccak256("SafenetGuard.AttestationTrailer.v1")` can be repointed at the new payload schema, exactly as the oracle-attestation migration itself repointed the v1 payload. This is the lowest-churn option (examples/docs already say `v1`). Flagged as an Open Question because `v1` is now merged to `main`, and the team may prefer a clean `v2` audit trail.

**`oracleData` is empty today.** Every current proposer passes `""`, so the commitment is `keccak256("")` and nothing changes semantically. The field exists, is authenticated, and can start carrying meaning later with zero wire-format change.

### Alternatives Considered

- **Option 1: carry `oracleData` in the trailer unsigned (no message change).** Rejected per the team's decision: it makes the field a malleable, unauthenticated hint, and any future use that must *trust* it would still need this bind change later - a second migration and a second protocol-wide re-coordination. Doing the bind now, while everything is unreleased and empty, is strictly cheaper.
- **Bind the raw `bytes oracleData` directly into the EIP-712 struct (not a hash).** Rejected: dynamic-length EIP-712 members are hashed as `keccak256(bytes)` under the spec anyway, and threading raw variable-length bytes through the event, the two Rust hashers, and the explorer typed-data mirror is more surface for no gain over committing to the hash ourselves.
- **Emit the raw `oracleData` in `OracleTransactionProposed` instead of the hash.** Rejected: no reconstructor needs the raw bytes (the oracle already has them via `postRequest`), and an unbounded event field is needless calldata/log cost.
- **Carry `oracleData` as a second, separate signature extension rather than a new field in the existing payload.** Rejected: `SignatureExtension` is explicitly single-extension, no nesting (`SignatureExtension.sol:30`); a second envelope would need a new transport format for no benefit over one extra `abi.encode` field.
- **Split the Solidity/validator/sentinel message change into three independent PRs (as the legacy-removal epic did).** Rejected: those removals were each independently valid at runtime; this is a matched wire-format pair whose halves are wrong in isolation, and the integration jobs prove it on every PR. Compiling independently (hand-written `sol!`/ABI mirrors) is not the same as integration-testing independently.
- **Keep `requestId` decoupled from the signed message (bind only the guard-side message).** Not possible: `Consensus` uses the one `message` value both as the coordinator's signing input and as the oracle `requestId`/`$attestations` key. Binding the signed message necessarily rekeys the oracle request. This is a real semantic change (see Open Questions), not an incidental one.

---

## Tech Specs

### Phase 1 - Bind `oracleData` into the consensus message across the coupled loop (Solidity + Rust validator + Rust sentinel)

One PR. Adds this epic file (transient; removed in the final phase). Self-consistent: every language's own unit tests and the cross-language integration scripts stay green because all three hashers change together.

**Solidity core**

- `contracts/src/libraries/ConsensusMessages.sol`:
  - Change `ORACLE_TRANSACTION_PROPOSAL_TYPEHASH` (35-36) to `keccak256("OracleTransactionProposal(uint64 epoch,address oracle,bytes32 safeTxHash,bytes32 oracleDataHash)")`; recompute the precomputed hex literal and update the `@custom:precomputed` comment (33).
  - Extend `oracleTransactionProposal` (120-136) with a trailing `bytes32 oracleDataHash` parameter; in the assembly, `mstore(add(ptr, 0x80), oracleDataHash)` and widen the struct-hash `keccak256(ptr, 0x80)` to `keccak256(ptr, 0xa0)` (the `0x1901`/domain-separator wrapping is unchanged). Update the NatSpec.
- `contracts/src/Consensus.sol` `proposeOracleTransaction` (347-360): compute `bytes32 oracleDataHash = keccak256(oracleData);` and pass it into `oracleTransactionProposal(...)` (353); add `oracleDataHash` to the `OracleTransactionProposed` emit (355-357). `message` is still both the coordinator signing input (358) and the `postRequest` `requestId` (359) - now binding `oracleData`.
- `contracts/src/interfaces/IConsensus.sol`: add `bytes32 oracleDataHash` to the `OracleTransactionProposed` event (110-117) with a matching `@param`; flip the `proposeOracleTransaction` `@param oracleData` NatSpec (304) from "not part of the signed message hash" to "committed to (as `keccak256`) in the signed message hash". Update `contracts/src/interfaces/IOracle.sol:30`'s parallel "not part of the signed message hash" note.
- Tests: `contracts/test/libraries/ConsensusMessages.t.sol` - update/extend the `oracleTransactionProposal` vector to include `oracleDataHash` and assert the new precomputed typehash. `contracts/test/Consensus.t.sol` - assert the emitted `oracleDataHash`, that `message`/`requestId` now depends on `oracleData`, and that two proposals for the same tx with different `oracleData` no longer collide on `AlreadyAttested` (see Open Questions).

**Rust validator mirror**

- `crates/validator/src/bindings.rs:148`: add `bytes32 oracleDataHash` to the `OracleTransactionProposed` `sol!` event.
- `crates/validator/src/consensus/hashing.rs`: add `oracle_data_hash` to `struct OracleTransactionProposal` (43); add the parameter to `oracle_transaction_proposal_hash` (127) and `oracle_transaction_packet_hash` (144); update the `sample_oracle_transaction_packet_hash` expected value (216) to the new Solidity hash.
- `crates/validator/src/state/transactions.rs`: thread `event.oracleDataHash` into the hash reconstruction at 135 and 175 (and the `handle_oracle_transaction_proposed` signature at 110 if the field must reach it).

**Rust sentinel mirror**

- `crates/sentinel/src/bindings.rs`: add `oracleDataHash` to `struct OracleTransactionProposal` (89) and the `OracleTransactionProposed` event (97).
- `crates/sentinel/src/hashing.rs`: add the `oracle_data_hash` parameter to `oracle_tx_proposal_hash` (82) and update the Solidity-parity test `oracle_tx_proposal_hash_parity` (146).
- `crates/sentinel/src/service.rs`: thread the new field through the `OracleTransactionProposed` handling at 115, 626, 781.

### Phase 2 - Guard: carry and verify `oracleData`

Stacked on Phase 1 (the guard tests build the expected message via the new `ConsensusMessages.oracleTransactionProposal` signature).

- `contracts/src/libraries/AttestationTrailer.sol`:
  - Change the payload schema to `abi.encode(uint64 epoch, address oracle, Secp256k1.Point groupKey, FROST.Signature signature, bytes oracleData)`. The head is now 256 bytes (224 static + one 32-byte offset word) plus a 32-byte length word and padded data, so the minimum payload (empty `oracleData`) is 288 bytes, not a fixed 224.
  - Replace the exact `require(payloadData.length == _PAYLOAD_LENGTH, MalformedAttestationTrailer())` (75) with an `abi.decode(payloadData, (uint64, address, Secp256k1.Point, FROST.Signature, bytes))` whose own offset/length validation over the `SignatureExtension`-bounded slice rejects malformed input; keep a clear-error guard (minimum-length or a try/wrapper) if the team wants `MalformedAttestationTrailer` preserved (see Open Questions). Add `bytes memory oracleData` to `decode`'s return tuple and NatSpec; update `_PAYLOAD_LENGTH` (33) and the header doc (12-13, 31).
  - Update the `TYPE_HASH` payload description; keep `keccak256("SafenetGuard.AttestationTrailer.v1")` per the redefine-v1 decision (or bump to `.v2` if the team prefers - Open Questions).
- `contracts/src/guard/SafenetGuard.sol` `checkTransaction` (200-230): capture `bytes memory oracleData` from `decode`; compute `bytes32 oracleDataHash = keccak256(oracleData);` and pass it into `ConsensusMessages.oracleTransactionProposal(_CONSENSUS_DOMAIN_SEPARATOR, epoch, oracle, safeTxHash, oracleDataHash)` (227-228) before `FROST.verify` (229). Update the comment (223-226) to note `oracleData` is now authenticated by the same signature.
- Tests: `contracts/test/libraries/AttestationTrailer.t.sol` - round-trip with empty and non-empty `oracleData`, plus a malformed-payload revert. `contracts/test/SafenetGuard.t.sol` - extend `_buildInlineAttestation` (175-186) to append `oracleData` to the payload and fold `oracleDataHash` into the message it signs; add a success case with non-empty `oracleData` and a tamper case (trailer `oracleData` altered so the rebuilt commitment breaks `FROST.verify`).

### Phase 3 - Guard documentation

Stacked on Phase 2.

- `contracts/src/guard/DESIGN.md:18-20`: update the trailer bullet to the five-field, variable-length payload; update the "attested message" bullet to `epoch + oracle + nonce-bound Safe tx hash + oracleData commitment`; note `oracleData` is now authenticated end-to-end.
- `contracts/src/guard/README.md:21,24,27,29,41,47,48,55`: update the payload layout diagram (`abi.encode(..., bytes oracleData)`), the `224`/`288`-byte figures (payload is now `>= 288`; total overhead `>= 352` for empty `oracleData`, growing with its length), the `checkTransaction`-outcomes paragraph, and the `OracleTransactionProposal` message description. Update the `v1`/version wording if Phase 2 bumps to `v2`.

### Phase 4 - Explorer client-side mirror

Depends on Phase 1 (new event field and hash definition). Independent of the guard phases; hand-written ABI/typed-data mirrors, so no compile coupling.

- `explorer/src/lib/oracle/hashing.ts:11-28`: add `oracleDataHash` to the `OracleTransactionProposal` typed-data types and to the function's inputs; keep it a faithful mirror of the updated `ConsensusMessages.oracleTransactionProposal`.
- `explorer/src/lib/consensus/abi.ts`: add `oracleDataHash` to the `OracleTransactionProposed` event fragment (and any event-selector list that must stay in sync).
- `explorer/src/lib/coordinator/signing.ts:121` (and callers of the oracle hashing path): thread `oracleDataHash` where the oracle proposal message is reconstructed for signing/verification. (The non-oracle `safeTxProposalHash`/`packets.ts` path is unrelated and untouched.)
- Update the affected explorer tests; manually verify in a browser (repo convention for frontend changes) that an oracle proposal's voting/verification UI still loads.

### Phase 5 - Reference example

Depends on Phase 1 (message) and Phase 2 (trailer payload). `examples/attest-safe-tx.ts`:

- Append `oracleData` (empty by default) to the `abi.encode` payload (241-271) as the new fifth field, and fold `keccak256(oracleData)` into the reconstructed `OracleTransactionProposal` message it signs (183 and around 54).
- Keep `TYPE_HASH` at `v1` (54) per the redefine decision, or bump to `v2` to match Phase 2.
- Reflect the change in the file's header walkthrough comment (4-12).

### Phase 6 - Remove this plan

Delete `epics/2026_08_11_bind_oracle_data.md` once Phases 1-5 are merged.

---

## Implementation Phases

| Phase | Summary | Depends on | Own PR |
|---|---|---|---|
| 1 | Bind `keccak256(oracleData)` into `OracleTransactionProposal`: Solidity core (`ConsensusMessages`/`Consensus`/`IConsensus` + event field + tests) **and** the Rust validator and sentinel hashing mirrors, together (integration-test-coupled). Adds this epic. | - | ✅ |
| 2 | Guard: carry raw `oracleData` in the trailer (`AttestationTrailer` five-field payload) and rebuild+verify the commitment in `SafenetGuard.checkTransaction`; guard forge tests | 1 | ✅ |
| 3 | Guard docs: `DESIGN.md`/`README.md` payload layout, overhead figures, message and trust-model wording | 2 | ✅ |
| 4 | Explorer client-side mirror: `oracle/hashing.ts` typed data, `consensus/abi.ts` event, `coordinator/signing.ts`, tests | 1 | ✅ |
| 5 | Reference example: `examples/attest-safe-tx.ts` payload + message | 1, 2 | ✅ |
| 6 | Remove this plan | 1-5 | ✅ |

Phase 1 is the single coupled unit. Phases 2 and 4 depend only on Phase 1 and can proceed in parallel (guard vs explorer); Phase 3 stacks on 2; Phase 5 needs both 1 and 2; Phase 6 closes it out.

---

## Open Questions and Assumptions

- **`requestId` semantics change.** Because the signed message equals the oracle `requestId` and the `$attestations` key, binding `oracleData` means the same Safe transaction proposed with different `oracleData` now produces distinct request ids (today they collide on `AlreadyAttested`). This is assumed desirable (different oracle input = different request), but it is a real protocol-behaviour change the team should confirm.
- **Redefine `v1` vs bump to `v2`** for `SafenetGuard.AttestationTrailer`. The plan redefines `v1` (guard unreleased, no on-chain consumers, lowest churn, matches how the oracle migration repointed v1). Since `v1` is now merged to `main`, the team may prefer `v2` for a clean audit trail; this flips one `keccak256("...v1"|"...v2")` string plus the docs/example wording.
- **Preserving `MalformedAttestationTrailer`.** With a variable-length payload there is no single valid length to assert; the plan leans on `abi.decode`'s built-in offset/length validation. If the team wants the custom error retained for a recognised-but-malformed payload, Phase 2 adds a minimum-length guard (`>= 288`) or a decode wrapper. Decide in Phase 2 review.
- **Does the sentinel actually recompute the proposal hash, or only read `requestId` from the oracle's own events?** `crates/sentinel/src/hashing.rs` has `oracle_tx_proposal_hash` with a Solidity-parity test, so the plan treats the sentinel as hash-coupled and includes it in Phase 1. If it turns out the sentinel never recomputes for correlation, it could be split out of Phase 1 - confirm during Phase 1 implementation.
- **`validator-integration` CI wiring.** The legacy-removal epic (`2026_08_09_...`) was adding a `validator-integration` job. Whether that job exists when this epic lands affects whether the validator half is *strictly* required in Phase 1's PR to keep CI green (the sentinel half is required regardless, since `sentinel-integration` runs today). The plan keeps all three in Phase 1 to be safe.
- **`oracleData` stays empty in all current call sites.** No existing proposer, script, or example passes non-empty `oracleData`; the plan does not introduce a meaningful payload, only the authenticated capacity to carry one. First real use is out of scope here.
- **Devnet/integration scripts** (`scripts/run_devnet.sh`, `scripts/run_sentinel_integration_test.sh`, and the validator integration script) may grep `OracleTransactionProposed` fields; confirm none parse positionally in a way the new event field breaks. Assumed low-risk (they key on the event name / indexed topics), flagged for Phase 1.