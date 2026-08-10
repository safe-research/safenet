# SafenetGuard — Design Decisions

Deliberate choices with their rationale, recorded so reviewers and auditors can distinguish "intended" from "oversight." They are not defects.

## Epoch trust model

- **Forest of `(groupKey, epoch)` pairs, kept forever, never pruned.** There is no single "active" epoch; multiple keys per epoch (reorg branches) are allowed. *Rationale:* the FROST per-participant secret shares are destroyed/rotated after an epoch, so a historical group key can never be reconstituted; keeping old pairs valid indefinitely is therefore not a practical risk. Storage grows monotonically, but each added pair requires a valid FROST signature.
- **A recorded historical key may attest future transactions and sign new rollover branches.** Accepted as a direct consequence of the above (shares no longer exist to abuse). It cannot replay past transactions, which the Safe nonce binds.
- **`updateEpoch` is permissionless** — the FROST signature is the authorization; the caller names the explicit parent pair; re-submitting a known pair is a no-op.
- **`rolloverBlock` is not checked against local `block.number`** — it is a Gnosis Chain block number, meaningless on the guard's chain, folded into the signed message only.

## Consensus binding

- **Consensus is Gnosis-only (chain id 100); the guard keeps a local copy of the epoch forest** (cross-chain calls are infeasible). The EIP-712 domain separator is immutable from constructor args; misconfiguration is unrecoverable (redeploy).

## Attestation (owner transactions)

- **Inline signature-extension trailer carrying `epoch + oracle + groupKey + signature` explicitly** (the forest has no epoch→key reverse lookup, and the guard needs the oracle to rebuild the consensus message). The non-standard encoding (wallets must be Safenet-aware) and the larger trailer are accepted trade-offs.
- **Carried as a `SignatureExtension`** — the reusable length-prefixed envelope `[payload][uint256 payloadLength][bytes32 TYPE_HASH]`. The terminal type hash (`keccak256("SafenetGuard.AttestationTrailer.v1")`) means a valid Safe-signature suffix cannot be mis-parsed, and a recognised trailer never falls through to the announcement path. `AttestationTrailer` is the typed codec over that transport (it asserts the fixed 224-byte attestation payload).
- **The attested message is the `OracleTransactionProposal`** (`epoch + oracle + nonce-bound Safe tx hash`). The `oracle` is trailer-supplied and the guard does not pin or allow-list it — trusting the group key and rebuilding the exact message the network signed. Which oracle is acceptable is enforced off-chain by the validators (they only attest results from an oracle they honour), consistent with the existing validator trust assumption.
- **Replay/ordering come from the Safe nonce** bound into that FROST-verified message; there is no spent-signature registry. The escape-hatch path is instead single-use and time-boxed.

## Escape hatch (announcements)

- **`announceTransaction` takes the full parameter struct** (not a bare hash) — signers see what they authorize and the guard derives the hash on-chain (so it cannot diverge from `checkTransaction`).
- **Nonce-free announcement hash** (excludes the Safe nonce and the Safe address; scoped by storage key) — keeps the hatch usable while unrelated transactions advance the Safe nonce.
- **Bounded, inclusive `[activeFrom, activeUntil]` window**, both durations immutable; packed into one slot (two `uint128`), with `WindowOverflow`/constructor bounds preventing absurd values.
- **Single-use; expired entries are renewable in place**; pending/active ones cannot be overwritten; `cancelAnnouncement` is immediate.
- **Consumption tracks the authorization path, not execution success** (a caught inner-call failure with non-zero `safeTxGas`/`gasPrice` still consumes the announcement). A lock/finalize/restore state machine was considered and deferred.
- **A relayer holding both a valid attestation and a matured announcement can choose the path**; the attestation path takes precedence and does not consume the announcement.
- **A single `TransactionAnnounced` event** — the full parameters are recoverable from the `announceTransaction` calldata, so the event carries only the hash and window.

## Structure & scope

- **Module-transaction guarding is intentionally not integrated** — deferred pending product requirements. Enabled Safe modules bypass this guard; deployers must prohibit modules or treat each as an explicit bypass.
- **The structural self-call gate is inlined** in the guard for explicitness, rather than extracted into a separate library.
- **Library-composed design** (`EpochRollover`, `TransactionAnnouncement`, `AttestationTrailer`): state/mechanism in libraries, FROST verification and domain events in the guard. `EpochRollover` epoch events are mirrored on `ISafenetGuard` for a single canonical integration ABI (they appear twice in the generated ABI; harmless — same topic).
