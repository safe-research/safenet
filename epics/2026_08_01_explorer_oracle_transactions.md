# Plan: Oracle Transaction Support in Explorer

Component: `explorer/` (settings, consensus event layer, new oracle voting-status module, transaction UI).

---

## Overview

Today the explorer only recognizes plain Safe transactions: it queries the `Consensus` contract for `TransactionProposed`/`TransactionAttested` logs (`explorer/src/lib/consensus/abi.ts`'s `transactionEventSelectors`) and renders their attestation progress. `Consensus` also supports **oracle-checked** transactions — proposed via `proposeOracleTransaction(oracle, oracleData, transaction)`, which emits `OracleTransactionProposed`/`OracleTransactionAttested` (carrying an `oracle` address) and forwards the request to an `IOracle` contract for approval before attestation. The explorer currently ignores these events entirely; oracle-checked proposals are invisible.

This epic adds oracle-transaction awareness to the explorer:

1. **Recognize oracle transactions.** Extend the Consensus event query to also decode `OracleTransactionProposed`/`OracleTransactionAttested`, keeping only the ones whose `oracle` address is trusted — either via a user-configured allow-list, or, absent one, because that oracle already has an `OracleTransactionAttested` on chain (untrusted/irrelevant oracle contracts are dropped, not just unlabeled).
2. **Configure the allow-list.** Add an `oracles` setting (comma-separated addresses, consistent with the explorer's existing flat-text-input settings form) alongside the existing `consensus`/`validatorInfo` settings.
3. **Show voting status.** For a recognized oracle proposal, derive the oracle's `requestId` and read back its resolution — at minimum "resolved approved/denied" (available for any `IOracle`), and, when the oracle is a `SentinelOracleV2`, the full `PENDING → FROZEN → RESOLVED_*` aggregate state — and render it in the transaction overview next to the existing attestation status.
4. **Show each sentinel's vote.** For a `SentinelOracleV2` request, list every sentinel that has cast a vote — state `committed` (voted, not yet revealed), `approved`, or `denied` — together with the sentinel's free-text `reason` when one was given. Only sentinels who have voted are shown; there is no roster/threshold concept, so no "missing vote" placeholders are rendered, and a request with zero votes shows nothing in this section (identical in spirit to the existing attestation view, but without the "all vs active" framing that view uses).

This is an explorer-only, additive change: no contract changes, no changes to plain (non-oracle) transaction rendering.

---

## Architecture Decision

### 1. Event layer — extend, don't replace

`explorer/src/lib/consensus/transactions.ts`'s `loadTransactionProposals` does a raw `eth_getLogs` with `topics: [transactionEventSelectors, safeTxHash ?? null, null, safe ? pad(safe) : null]`. `OracleTransactionProposed`/`OracleTransactionAttested` share the same indexed-topic layout (`safeTxHash`, `chainId`, `safe`), so they slot into the existing topic filter by adding their selectors to `transactionEventSelectors` — no change to the query shape itself. `oracle` is **not indexed** on either event, so "only the ones with the oracle address [in the allow-list]" cannot be expressed as a log topic filter; it is applied after `parseEventLogs` decodes `log.args.oracle`, against the addresses configured in settings.

`TransactionProposal` gains an optional `oracle: Address | null` field. Everywhere else in the type (`ProposalStatus`, attestation linking) is reused unchanged — an oracle proposal still becomes `PROPOSED`/`ATTESTED`/`TIMED_OUT` via the same attestation-matching logic that already exists, just sourced from `OracleTransactionProposed`/`Attested` instead of `TransactionProposed`/`Attested`.

One correction needed while touching this code: the existing attestation map key is `` `${safeTxHash}:${epoch}` ``. A plain and an oracle-checked proposal for the same `safeTxHash` in the same epoch (contrived, but not prevented by the contract) would collide. The key is extended to include a discriminator (`oracle ?? "plain"`) so the two event families never cross-match.

### 2. Trust boundary: configured allow-list, falling back to on-chain attestation

Chosen: an oracle proposal is **dropped** from results unless its `oracle` is trusted. Trust is resolved as:

- the configured `oracles` allow-list, when it is non-empty (explicit config is authoritative); otherwise
- **derived from consensus itself**: any `oracle` address that appears on an `OracleTransactionAttested` log is implicitly trusted. That event only exists once the validator set's FROST attestation quorum has signed off on a transaction proposed through that specific oracle — i.e. the oracle's approval has already cleared the same consensus process that authorizes plain transactions. That is a stronger trust signal than an operator hand-typing an address into settings, so it is a sound default with zero configuration.

In practice: with no `oracles` configured, a fresh install is not blind to oracle transactions — an oracle "earns" visibility the moment one of its transactions is attested, and from then on its other proposals (including ones still awaiting attestation) are shown too. A brand-new, never-attested oracle's very first `OracleTransactionProposed` is not shown until that same proposal is attested — consistent with how an unattested plain proposal already renders as `PROPOSED`/`TIMED_OUT` rather than being hidden, except here the *oracle itself*, not just the proposal, is unproven.

**Known limitation, accepted:** because `loadTransactionProposals` queries a bounded block window (`fromBlock`/`toBlock`, sized by `maxBlockRange`), the attested-derived trust set is built from attestations visible *in that same window*, not all-time history. An oracle attested only outside the current window won't re-derive trust for new proposals inside it until it attests again inside the window or is added to the explicit list. This is the same windowed-log-query trade-off `maxBlockRange` already makes elsewhere (e.g. `safe`-filtered queries already document that `TransactionAttested` topic-filtering can lag for the same reason) — not a new class of behavior.

Alternative considered — show all oracle proposals, only enrich recognized ones with voting status: rejected. It re-adds exactly the "trust whatever shows up on-chain" behavior the rest of the explorer's settings model deliberately avoids, and clutters the list with proposals the deployment operator has no way to evaluate.

### 3. Allow-list format — comma-separated, in `Settings`

The explorer's settings are a flat Zod-validated object in `localStorage` (`explorer/src/lib/settings.ts`), edited via single-line text inputs (`ConsensusSettingsForm.tsx`); there is no existing multi-value input in the UI. The validator side has a precedent for multi-address config (`SENTINEL_BLOCKLIST`, `ALLOWED_ORACLES` in `validator/src/types/schemas.ts`), but it's a **JSON array string** in an env var — reasonable for a `.env` file, awkward to hand-type into a form field. Comma-separated addresses in a single text input is a new convention for this codebase, but matches how the rest of the explorer's settings form is built (one line, one field, no JSON), which is why the epic prompt calls for it explicitly.

`oracles: Address[]` is added to `settingsSchema`/`DEFAULT_SETTINGS`, sourced from a new `VITE_DEFAULT_ORACLES` build-time default (comma-separated, validated in `vite.config.js` like `VITE_DEFAULT_CONSENSUS` already is). The settings-form schema parses the field as `z.union([z.array(checkedAddressSchema), z.string()])` — the union exists because `defaultValues` supplies the already-parsed `Address[]` from `Settings` while the user edits it as raw comma-separated text — `.transform(...)` splits on `,`, trims, drops empty segments, and validates each address, mirroring the existing `numberOrStringAsNumber` pattern used for `maxBlockRange`/`refetchInterval`.

### Alternatives Considered

- **A — JSON-array text field (mirror the validator's `jsonStringToValue` convention).** Rejected: forces users to hand-type `["0x...","0x..."]` into a plain `<input>`, worse UX than every other field on that form, for the sake of an internal-consistency argument that only holds across workspaces, not within the explorer's own settings page.
- **B — Comma-separated string (chosen).** Matches the rest of the explorer settings form; explicit ask from the epic prompt.
- **C — Enrich instead of filter (show all oracle proposals, badge only known ones).** Rejected, see above — breaks the explorer's "only render what's configured/trusted" model.
- **D — Filter oracle proposals via log topics instead of post-decode.** Not possible: `oracle` isn't an indexed parameter on `OracleTransactionProposed`/`OracleTransactionAttested`, so there is no topic to filter on; filtering must happen after decoding.
- **E — Explicit allow-list only; empty means hide all oracle proposals.** The initial design for this epic. Rejected as the sole behavior: it leaves a fresh install blind to every oracle transaction until an operator manually populates addresses, even though the chain already provides a trust signal — attestation — for free.
- **F — Empty allow-list means show all oracle proposals unfiltered.** Rejected: same problem as alternative C — no vetting, surfaces arbitrary or malicious oracle contracts by default.
- **G — Derive trust from `OracleTransactionAttested` as the fallback source of truth when no explicit list is configured (chosen).** Uses the chain's own consensus signal instead of requiring config, while still refusing to show proposals from an oracle that has never cleared attestation.

### 4. Per-sentinel votes: `SentinelOracleV2` only, discovered from events, no roster needed

`SentinelOracle` exists in two on-chain versions: **V1** (`SentinelOracle.sol` / `SentinelOracleCommitments.sol`, what the validator's `SentinelService` automates today via `commitApprove`/`commitDeny`) reveals a sentinel's vote direction the instant it's cast, with no reveal step and no reason field. **V2** (`SentinelOracleV2.sol` / `SentinelOracleCommitmentsV2.sol`) is a commit/reveal scheme: `commit(requestId, commitHash)` emits `event Committed(bytes32 indexed requestId, address indexed sentinel, uint256 bondAmount)` (direction hidden), and `reveal(requestId, approve, salt, reason)` emits `event Revealed(bytes32 indexed requestId, address indexed sentinel, bool approved, uint256 bondAmount, string reason)`.

V1 is deprecated and slated for removal, so this epic targets **V2 only** — there is no dual-version detection, no fallback to a V1 ABI, and no V1 event handling anywhere in the plan. `getRequest`/`getCommitment`/`Committed`/`Revealed` are read exclusively against the V2 shape (`Request`: `proposer, fee, bondTarget, commitDeadline, revealDeadline, state, committedCount, revealedCount, approveSentinelCount, denySentinelCount`). The three requested vote states map directly onto `SentinelOracleCommitment.Vote`: `PENDING` → `committed` (not yet revealed), `APPROVED`, `DENIED` — and `reason` is a genuine per-vote field the V2 contract stores, not something the explorer infers.

`Committed`/`Revealed` both index `requestId` **and** `sentinel`, so the full per-vote list for a request is discoverable with a plain `eth_getLogs` filtered by the `requestId` topic — no need to know the sentinel set in advance, and nothing to model in settings. This directly satisfies "only show collected votes, don't show missing ones": the log set only ever contains sentinels who acted. If a configured oracle's `getRequest` doesn't match the V2 ABI at all (e.g. it's a plain `IOracle` like `AlwaysApproveOracle`, or a still-running V1 deployment), the per-sentinel breakdown is simply omitted — only the generic `OracleResult`-derived aggregate status (Architecture Decision's voting-status design) is shown.

Alternative considered — support both V1 and V2 ABIs (try V2, fall back to V1): rejected per explicit instruction — V1 is being removed, so building and maintaining a second ABI/decode path for it is pure churn.

Alternative considered — model an explicit sentinel roster in settings (mirroring `oracles`) and show "voted"/"missing" like `SafeTxAttestationStatus`'s `ValidatorList`: rejected per explicit instruction — there's no threshold/roster concept to complete here, and the indexed events already make a roster unnecessary.

---

## Tech Specs

### Settings (`explorer/src/lib/settings.ts`, `ConsensusSettingsForm.tsx`, `vite.config.js`, `.env.sample`)

- `Settings.oracles: Address[]` (default `[]`), backed by `VITE_DEFAULT_ORACLES` (comma-separated, empty by default). Unlike `VITE_DEFAULT_RELAYER`'s "empty disables the feature" pattern, an empty `oracles` list does not disable oracle-transaction recognition — it switches the trust source to attested-derived (see Architecture Decision §2) rather than hiding everything or trusting everything.
- New form field in `ConsensusSettingsForm.tsx`: a single text input, label "Oracle Addresses (comma-separated)", parsed/validated per the union-schema transform described above.

### Consensus event layer (`explorer/src/lib/consensus/abi.ts`, `transactions.ts`)

- `consensusAbi` gains the `OracleTransactionProposed`/`OracleTransactionAttested` event fragments (same shape as `IConsensus.sol`).
- `transactionEventSelectors` becomes 4 selectors (`TransactionProposed`, `TransactionAttested`, `OracleTransactionProposed`, `OracleTransactionAttested`).
- `loadTransactionProposals` gains an `oracles: Address[]` param (from settings). After `parseEventLogs`:
  - Build `trustedOracles`: if `oracles` is non-empty, that's the set; otherwise, the set of every distinct `args.oracle` value found on `OracleTransactionAttested` logs in the just-decoded batch.
  - `OracleTransactionProposed`/`Attested` logs whose `args.oracle` is not in `trustedOracles` are discarded before they reach the proposal-building step (an oracle's own `OracleTransactionAttested` log is trivially a member of its derived set, so it is never filtered out by its own presence).
  - Surviving oracle logs feed the same proposal/attestation construction as today, tagging the result with `oracle: log.args.oracle`; plain logs keep `oracle: null`.
  - The attestation map key becomes `` `${safeTxHash}:${epoch}:${oracle ?? "plain"}` ``.
- `TransactionProposal`/`TransactionProposalWithStatus` gain `oracle: Address | null`. Consumers (`useRecentTransactionProposals`, `useSafeTransactionProposals`, `useProposalsForTransaction`) are unaffected beyond the new field being present.

### Voting-status data layer (new `explorer/src/lib/oracle/`)

- `hashing.ts` — `oracleRequestId({ chainId, consensus, epoch, oracle, safeTxHash })`, an EIP-712 `hashTypedData` call reproducing `Consensus.domainSeparator().oracleTransactionProposal(epoch, oracle, safeTxHash)` (`ConsensusMessages.sol`). This is the same computation the validator already performs in `validator/src/consensus/verify/oracleTx/hashing.ts` (`oracleTxProposalHash`) — there is no package shared between `validator/` and `explorer/` today (each already keeps its own copy of the Consensus ABI/hashing), so this is a small, independently-tested duplication rather than a new shared package. Unit tests assert parity against known vectors from the validator's existing test suite.
- `abi.ts` — read-only ABI fragments for `SentinelOracleV2`'s `getRequest(bytes32) -> Request` (`proposer, fee, bondTarget, commitDeadline, revealDeadline, state, committedCount, revealedCount, approveSentinelCount, denySentinelCount`), its `Committed`/`Revealed` events, and the generic `IOracle.OracleResult` event. No V1 ABI — V1 is deprecated and out of scope (Architecture Decision §4).
- `votingStatus.ts` — `loadVotingStatus({ provider, oracle, requestId })`, the **aggregate** status:
  1. Attempt `getRequest(requestId)` against the `SentinelOracleV2` shape. On success, map `Request.state` (`PENDING | FROZEN | RESOLVED_APPROVED | RESOLVED_DENIED | TIMED_OUT`) plus vote counts (`approveSentinelCount`/`denySentinelCount`) to a `VotingStatus`.
  2. If the call reverts (oracle isn't `SentinelOracleV2`-shaped — a plain `IOracle` like `AlwaysApproveOracle`/`SimpleOracle`, or a still-running deprecated V1 deployment), fall back to an `eth_getLogs` lookup of `OracleResult` by `requestId` topic against the oracle contract: `approved: true/false` once emitted, otherwise `PENDING`.
  3. Returns `VotingStatus = { kind: "sentinel"; state: ...; approveCount; denyCount } | { kind: "generic"; approved: boolean | null } | null` (`null` while `oracle`/`requestId` aren't yet known, e.g. proposal still loading).
- `votes.ts` — `loadSentinelVotes({ provider, oracle, requestId })`, the **per-sentinel** breakdown, only called when `votingStatus.kind === "sentinel"` (a generic oracle has no per-vote events to read): `eth_getLogs` for `Committed`/`Revealed`, both filtered by the `requestId` topic. Build one entry per sentinel: those only in `Committed` → `{ sentinel, state: "committed" }`; those in `Revealed` → `{ sentinel, state: approved ? "approved" : "denied", reason }`. Returns `SentinelVote[]`, ordered by block/log-index (first vote first); empty array when nobody has voted — the UI renders nothing in that case, per the "no votes, nothing to show" requirement.
- `useVotingStatus(oracle, requestId)` and `useSentinelVotes(oracle, requestId)` hooks, following the polling pattern of `useAttestationStatus` (`useSigningProgress.ts`), gated by the settings `refetchInterval`.

### UI (`SafeTxProposals.tsx`, new `VotingStatusBadge.tsx`, new `SentinelVoteList.tsx`)

- `VotingStatusBadge` renders the `VotingStatus` union: `sentinel` states map to badge variants (`PENDING`/`FROZEN` → pending, `RESOLVED_APPROVED` → positive, `RESOLVED_DENIED`/`TIMED_OUT` → error); `generic` maps `approved`/`!approved`/`null` the same way with a plainer label (no vote counts to show).
- `SentinelVoteList` renders `SentinelVote[]` as a simple list (not the `ValidatorList` avatar-grid pattern, which assumes a known "all" roster to show gaps against): one row per sentinel with an inline address, a small badge for `committed`/`approved`/`denied`, and the `reason` text when present. Renders nothing (not an empty state message) when the list is empty.
- `SafeTxProposal` (the per-proposal box in `SafeTxProposals.tsx` — the "Transaction Details" overview) renders, only when `proposal.oracle != null`: a new "Voting:" row directly under the existing "Status:" row (`VotingStatusBadge`), followed by `SentinelVoteList` when the oracle is `SentinelOracle`-shaped. Non-oracle proposals render exactly as they do today.

### Out of scope for this epic

- Surfacing oracle/voting status in the compact list row (`TransactionListRow.tsx`) — the epic prompt specifically calls out the "transaction overview" (detail page); the compact list keeps its current columns.

---

## Implementation Phases

Each phase is its own PR. Phases 1 and 3 have no dependency on each other and can be worked in parallel; phase 2 depends on phase 1 (reads the new setting); phase 4 depends on phase 3 (shares `abi.ts`/`hashing.ts` and the version-tagged `VotingStatus`); phase 5 depends on phases 2, 3, and 4.

- **Phase 0 — this epic.** Intent, chosen design, and plan for reviewer context before any code.
- **Phase 1 — Oracle allow-list setting.** `Settings.oracles`, `VITE_DEFAULT_ORACLES` wiring in `vite.config.js`/`.env.sample`, the comma-separated form field in `ConsensusSettingsForm.tsx`, and the union/transform schema. No consumers yet — purely additive config, safe to land standalone.
- **Phase 2 — Recognize oracle transaction events.** `consensusAbi` event additions, `transactionEventSelectors` extension, `loadTransactionProposals` decoding + allow-list filtering + the attestation-key fix, `TransactionProposal.oracle` field. Existing tests updated to cover the discard-if-unlisted and no-collision behavior; no UI change (oracle proposals appear in lists like any other proposal, just now present).
- **Phase 3 — Aggregate voting-status data layer.** New `explorer/src/lib/oracle/` (`hashing.ts`, `abi.ts`, `votingStatus.ts`) plus `useVotingStatus`. Unit-tested in isolation (hash parity vectors, `SentinelOracleV2` `getRequest` happy path, revert-to-generic path for non-`SentinelOracleV2` oracles) — no UI consumer yet, reviewable purely as data-layer correctness.
- **Phase 4 — Per-sentinel vote breakdown data layer.** `votes.ts` (`loadSentinelVotes`) plus `useSentinelVotes`, built on Phase 3's `abi.ts`. Unit-tested against `Committed`+`Revealed` log shapes (committed-not-revealed, approved-with-reason, denied-with-reason), including the "nobody voted → empty array" case. No UI consumer yet.
- **Phase 5 — Voting status in the transaction overview.** `VotingStatusBadge` + `SentinelVoteList` + the new "Voting:" section in `SafeTxProposals.tsx`, wired to `useVotingStatus`/`useSentinelVotes` via the proposal's `oracle` field and the Phase 3 `requestId` derivation.
- **Phase 6 — Remove this epic spec file**, once phases 1–5 have shipped.

---

## Open Questions and Assumptions

- **Assumption:** "the event that is being listened to should be the ones with the oracle address" means oracle proposals are filtered to the configured allow-list, not merely labeled — confirm this matches intent before Phase 2, since it changes what operators see (proposals from unlisted oracles disappear rather than showing unlabeled).
- **Assumption:** "transaction overview" refers to the per-proposal detail box on the `/safeTx` page (`SafeTxProposals.tsx`), not the compact recent-transactions list row. If the list row should also carry a voting indicator, that's a small additional Phase 5 change, not a new phase.
- **Decided:** an empty `oracles` allow-list does not hide oracle proposals; trust falls back to "has this oracle produced an `OracleTransactionAttested` [in the queried window]" (Architecture Decision §2).
- **Accepted limitation:** attested-derived trust is scoped to the currently queried block window (bounded by `maxBlockRange`), not all-time chain history — an oracle attested only outside that window won't retroactively unlock new proposals inside it until it attests again in-window or is added to the explicit list. Flag if a persistent/indexed trust cache turns out to be needed sooner than expected.
- **Decided:** per-sentinel votes are shown only for `SentinelOracleV2` requests, discovered from indexed `Committed`/`Revealed` logs rather than a configured sentinel roster; only sentinels who voted appear, with no "missing" placeholders (Architecture Decision §4). V1 is deprecated and out of scope — no V1 ABI, decoding, or fallback path is built. This deployment's validator (`SentinelService`) still only automates V1 today (V2 has a deploy script and integration test but isn't wired up in the validator yet), so per-sentinel votes will show nothing until a `SentinelOracleV2` deployment is actually in use — the aggregate voting-status badge still degrades to the generic `OracleResult`-derived status in the meantime.
