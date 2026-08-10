# Oracle-based attestation for SafenetGuard

## Intent

Migrate `SafenetGuard`'s attestation path from the basic transaction-proposal message to the
**oracle** transaction-proposal message, so the guard verifies the attestations Safenet actually
produces now that the protocol has consolidated on oracle-gated proposals. The other transaction-proposal
method is being retired.

## Background

Safenet attests to a Safe transaction by having its FROST group threshold-sign a consensus message over
the nonce-bound Safe transaction hash. Two message shapes exist in `ConsensusMessages`:

- `TransactionProposal(uint64 epoch, bytes32 safeTxHash)` — the original ("basic") shape.
- `OracleTransactionProposal(uint64 epoch, address oracle, bytes32 safeTxHash)` — binds the **oracle**
  contract the proposal was gated by. Its hash doubles as the oracle `requestId`.

Flow: `Consensus.proposeOracleTransaction(oracle, oracleData, tx)` signs the oracle message and posts a
request to `IOracle(oracle)`. The oracle emits `OracleResult(requestId, …, approved)`. Validators watch
that result **off-chain** and only contribute their FROST share when `approved` — so the oracle is an
input to the validators' signing decision, and the message records which oracle gated it. The coordinator
changes (decline tracking, dispute reasons) concern the signing ceremony only; the final
`FROST.Signature` shape and the guard's verification math are unchanged.

Today the guard reconstructs the **basic** message, so it would reject every oracle-based attestation.

## What changes

**Contract**
- `AttestationTrailer` payload re-laid out from `abi.encode(uint64 epoch, Secp256k1.Point groupKey,
  FROST.Signature signature)` (192 bytes) to `abi.encode(uint64 epoch, address oracle,
  Secp256k1.Point groupKey, FROST.Signature signature)` (224 bytes). The `TYPE_HASH`
  (`SafenetGuard.AttestationTrailer.v1`) is **kept** — this is a re-layout, not a version bump, because
  `v1` is unreleased and there is no consumer to stay compatible with.
- `SafenetGuard.checkTransaction` decodes the `oracle` and verifies the FROST signature against
  `ConsensusMessages.oracleTransactionProposal(domain, epoch, oracle, safeTxHash)`.
- The basic `transactionProposal` path is **removed** (full migration, no back-compat).

**Tests**
- `AttestationTrailer.t.sol` and the guard's attestation test helpers/paths updated to the 224-byte
  oracle payload and the oracle message.
- Guard-level malformed-trailer coverage for the `payloadLength != 224` shape.

**Docs**
- `guard/README.md` and `guard/DESIGN.md` updated to the oracle message, the 224-byte payload and the
  trust model below.

## Trust model & decision

**The `oracle` is carried in the trailer; the guard does not pin or allow-list it.** The guard trusts the
`(groupKey, epoch)` forest and rebuilds the exact message the network signed, using the oracle the trailer
names. *Which* oracle is acceptable is enforced **off-chain by the validators** (they only act on an
`OracleResult` from an oracle they honour), consistent with the existing trust assumption that a threshold
of validators is honest. No new on-chain guard state, no oracle immutable, no allow-list.

### Alternatives considered

- **(a) Immutable oracle pinned in the guard** — strongest (a rogue proposer naming a weak oracle could
  not produce an attestation the guard accepts), and would keep the trailer at 192 bytes, but forces a
  guard change on any oracle rotation.
- **(c) On-chain allow-list** — survives rotation, at the cost of new governance/state on the guard.
- **(b) Oracle from the trailer (chosen)** — no extra guard state or governance; the guard's security
  reduces to the validator honesty assumption Safenet already relies on. Selected because we already trust
  the validators and want to avoid guard-side oracle management.

## Non-goals (this stack)

- The example relayer script (`examples/attest-safe-tx.ts`) — follow-up.
- The Certora formal-verification suite — reworked for the oracle model in a later stack.
- Backward compatibility and redeployment concerns — not applicable; the guard is unreleased (only tested
  and under active development).

## PR breakdown (this stack)

1. **Epic** (this document).
2. **Contract + tests** — `AttestationTrailer`, `SafenetGuard.checkTransaction`, and the associated test
   updates.
3. **Docs** — `guard/README.md` and `guard/DESIGN.md`.
4. **Epic removal** — this document is transient scaffolding for the stack review and is deleted here.