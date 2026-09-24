# Plan: Explorer for the Safenet Aegis launch

Component: `explorer/` (NPM package). No contract, service or devnet changes.

---

## Overview

Aegis is the code name of the Safenet release that follows Safenet Beta. The explorer needs two changes for the launch.

**Branding.** The header logo reads "Safenet BETA". It becomes the Aegis logo, and the home-page tagline changes from "Explore the future of transaction security!" to "Explore the onchain defence protocol". The explorer keeps the name "Safenet Explorer".

**Arbitration resolution states.** #898 made the transaction lists show the oracle's verdict (`APPROVED`, `DENIED`) alongside `PROPOSED`, `ATTESTED` and `TIMED_OUT`. It reads that verdict from `OracleResult`, which `SentinelOracle` never emits for a split vote. So a disputed proposal shows `TIMED_OUT` in every list for its whole life: while the Security Council deliberates, after it rules, after it declines the request as out of scope, and after the arbitration deadline expires. The transaction page shows `ARBITRATING` from the oracle's request state, and after a ruling it looks the same as a unanimous vote.

The work splits into these PRs:

1. **Branding**: the Aegis logo, the new tagline, and removal of an unused Beta logo file.
2. **Arbitration statuses in the lists**: read the dispute events in the query #898 added for verdicts, and derive four new proposal statuses.
3. **Arbitration details on the transaction page**: when the dispute started, its deadline, the arbitrator, the outcome and the Council's reason.
4. **Cleanup**: remove this plan.

Phase 1 is independent and can land in parallel with Phases 2 and 3. Phase 3 builds on Phase 2.

---

## Architecture Decision

### Dispute events join the existing verdict query

`SentinelOracle` emits these events for a disputed request. Like `OracleResult`, each has `requestId` as its first indexed topic.

| Event | Emitted when |
| --- | --- |
| `DisputeTriggered(requestId, deadline)` | `finalize` finds both an approve and a deny side and freezes the request (`FROZEN`). |
| `DisputeResolved(requestId, outcome, slashed, context)` | The Council rules through `resolveDispute`. `outcome` is `RESOLVED_APPROVED` (secure) or `RESOLVED_DENIED` (insecure). |
| `DisputeOutOfScope(requestId, context)` | The Council declines the request as out of scope through `markOutOfScope` (Charter §3.9). |
| `ArbitrationTimedOut(requestId)` | Anyone calls `timeoutArbitration` after the deadline, and no ruling was made. |

`loadOracleVerdicts` (#898) fetches `OracleResult` with one `eth_getLogs` against the trusted oracles, filtered by request ID when the query is scoped to a transaction or a Safe. The four dispute event selectors join `OracleResult` in the same `topics[0]` filter, so arbitration adds no RPC call. `votes.ts` already reads `Committed` and `Revealed` the same way.

A request either resolves directly (`OracleResult`) or freezes (`DisputeTriggered`), never both. `finalize` returns before emitting `OracleResult` when it freezes a request, and `resolveDispute` emits only `DisputeResolved`. So the status derivation needs no precedence rule between the two.

### A disputed proposal is final without an attestation

The Charter (Article I, "Council and protocol boundaries") says a transaction that enters arbitration is not eligible for validator attestation, whatever the ruling. The code matches: validators act only on `OracleResult`, and drop the signing round after `oracle_timeout`. So each arbitration status either waits on the Council or is final. None waits on the validators, and none turns into `TIMED_OUT` through `signingTimeout`.

### Outcome events are read up to the latest block

The Safe page pages backwards through history by moving `toBlock` into the past (`useSafeTransactionProposals`), and #898 reads verdicts within the same `[fromBlock, toBlock]` window. A verdict lands a few blocks after its proposal, so the window almost always contains it. A Council ruling can land weeks later (the Charter allows four weeks, §2.17), well after an older page's `toBlock`. The oracle outcome query therefore uses `latest` as its upper bound, so an older page still shows how a dispute ended. The consensus query keeps its window, and statuses are still evaluated at the page's `toBlock` as they are today.

### Four new statuses, labelled with Charter terms

`ProposalStatus` grows from five values to nine:

| Status | Badge label | Colour (`Badge` variant) | Meaning |
| --- | --- | --- | --- |
| `ARBITRATING` | ARBITRATING | yellow (`pending`) | Split vote. Waiting for the Council, or for someone to time the dispute out. |
| `SECURE` | SECURE | cyan (`info`) | The Council ruled the transaction `secure`. No attestation will follow. |
| `INSECURE` | INSECURE | red (`error`) | The Council ruled the transaction `insecure`. |
| `NO_RULING` | NO RULING | orange (`warning`) | The Council declined the request as out of scope, or the deadline passed and someone timed it out. |

- **Labels.** The list's status column is 5rem wide, which fits about 11 characters of badge text. "RULED INSECURE" and "OUT OF SCOPE" would overflow it. `secure` and `insecure` are the Charter's words for a ruling (§2.13), and they keep Council rulings apart from the sentinels' `APPROVED` and `DENIED`.
- **`NO_RULING` covers both out-of-scope and a timeout.** The Charter gives them the same consequences for bonds, fees and slashing: "only the timing differs" (§3.9). The transaction page says which one happened.
- **Colours** run from red (failure) to green (success) and use existing tokens only. `ARBITRATING` keeps the yellow the transaction page already uses for it. `SECURE` reuses the cyan #898 chose for `APPROVED`, because it means the same thing: a positive verdict with no attestation yet, or here never. `INSECURE` shares red with `DENIED`. `NO_RULING` sits between failure and in progress, in the orange of the existing `warning` variant.
- **A dispute past its deadline stays `ARBITRATING`.** On chain it is still `FROZEN`, and the Council can still rule until someone calls `timeoutArbitration`, because `resolveDispute` checks only the state. The transaction page shows that the deadline has passed.

### The oracle badge on the transaction page stays as it is

`VotingStatusBadge` shows the oracle request's own state from `getRequest`: `APPROVED` or `DENIED` after a ruling, and `TIMED OUT` after an out-of-scope decision or a timeout. That is accurate for the oracle. The proposal's `Status` row, directly above it, now carries the arbitration outcome, so the badge doesn't need to change.

### Council reasons are shown as raw text

`context` in `DisputeResolved` and `DisputeOutOfScope` is free text or an IPFS CID (Charter §3.9). The explorer shows the string exactly as emitted, as plain React text: no link detection, no IPFS gateway, no Markdown. Resolving CIDs and linking to the Charter are out of scope. Sentinel vote reasons stay as they are today, raw strings too.

### Alternatives Considered

- **Read each request's state with `getRequest` instead of events.** This gives `FROZEN` and the deadline directly, but costs one call per unresolved proposal on every list refresh (or a multicall). It would still need the events for the Council's reason and to tell out-of-scope from a timeout. Rejected: the event query already exists.
- **Recognise a ruling from vote counts.** A resolved request with both approve and deny votes can only come from a ruling, so `getRequest` alone could relabel the oracle badge. Rejected for the lists, for the same per-proposal cost. Not needed on the transaction page, where the `Status` row now shows the outcome.
- **Separate `OUT_OF_SCOPE` and `ARBITRATION_TIMED_OUT` list statuses.** Neither label fits the status column, and the Charter treats the two as the same outcome. The transaction page tells them apart.
- **Also read `RequestTimedOut`** (nobody voted or revealed), which #898 leaves out. It is not an arbitration state, and the existing `TIMED_OUT` inference covers it, so it stays out of this epic.
- **One source for the release name.** "Aegis" appears only in the logo artwork and the logo's accessible label, so changing it by hand in the next release is simpler than a mechanism.

---

## User Flow

A user opens the explorer, sees `ARBITRATING`, `SECURE`, `INSECURE` or `NO RULING` on a disputed proposal in the home or Safe list, and opens it to see how the dispute went.

### Page Layout

List row on the home and Safe pages. Only the status badge changes:

```text
+-------------+--------------+--------------+-----------------------+----------+
| Gnosis      | 0x1234..abcd | 0xfe8b..4028 | CALL to 0x9876..5432  | 2h ago   |
| ARBITRATING |              |              | 1.5 ETH               | #4839775 |
+-------------+--------------+--------------+-----------------------+----------+
```

Proposal box on the transaction page. The `Arbitration` rows appear only for a disputed proposal:

```text
Proposal #1 (i)
Status:                                                    INSECURE
Oracle (i):                                                  DENIED
Votes:                                        Carol [ok]  Dave [x]
Arbitration:
    Started:     Block 48308388 at 21/09/2026, 10:12:03   Explorer Tx
    Deadline:    Block 48358788 (passed)
    Arbitrator:  0xE682..5cc8
    Outcome:     Ruled insecure at Block 48400000         Explorer Tx
    Reason:      <the Council's context string, verbatim>
    Transactions that enter arbitration are never attested, whatever the
    ruling. To execute this one, propose it again in a later epoch or use
    the escape hatch.
Proposed:                          Block 48308377 at ...   Explorer Tx
Attested:                                                          -
```

- `Outcome` reads "Pending", "Ruled secure", "Ruled insecure", "Out of scope" or "Timed out without a ruling".
- `(passed)` shows only while there is no outcome and the current block is past the deadline.
- `Reason` shows only for a ruling or an out-of-scope decision, and only when the string is not empty.
- The closing note is an open question (see below).

---

## Tech Specs

### Branding (Phase 1)

- Rename `src/components/common/SafenetBetaLogo.tsx` to `SafenetLogo.tsx` (export `SafenetLogo`), and update the import in `src/components/Header.tsx`. The name no longer carries a release, so the next release only swaps the artwork.
- Take the two paths from the design file `logo-Black.svg` (viewBox `0 0 435 128`). Its mark and wordmark match today's logo, and only the release label changes. Keep `width="109" height="32"`, `fill="currentColor"` and the `fill-title` class, so the logo still follows the theme (black on light, white on dark). Set `aria-label="Safenet Aegis"`.
- Delete `src/assets/SafenetBeta.svg`, a copy of the Beta logo that has never been imported.
- Change the tagline in `src/routes/index.tsx` to "Explore the onchain defence protocol".
- Unchanged: the page title and meta tags ("Safenet Explorer"), the favicons and app icons (same mark), `public/og-image.png` (still shows BETA until design supplies a new one), and the `safenet-beta-data` default URLs (that repository is renamed later).
- Test: `Header.test.tsx` checks the logo's accessible name is "Safenet Aegis".

### Arbitration statuses in the lists (Phase 2)

`src/lib/oracle/abi.ts`

- Add `DisputeTriggered`, `DisputeResolved` (`outcome` as `uint8`), `DisputeOutOfScope` and `ArbitrationTimedOut` to `sentinelOracleAbi`, matching `contracts/src/SentinelOracle.sol`.
- Replace `oracleResultEventSelector` with `oracleOutcomeEventSelectors`: `OracleResult` plus the four dispute events.

`src/lib/consensus/transactions.ts`

- New type, attached to `TransactionProposalWithStatus` as `arbitration: Arbitration | null`:

  ```ts
  export type Arbitration = {
    triggeredAt: ExecutionLink;
    deadline: bigint;
    outcome:
      | { kind: "ruled"; secure: boolean; context: string; at: ExecutionLink }
      | { kind: "outOfScope"; context: string; at: ExecutionLink }
      | { kind: "timedOut"; at: ExecutionLink }
      | null;
  };
  ```

- `loadOracleVerdicts` becomes `loadOracleOutcomes`. It returns, per request ID, either a verdict (as today) or an `Arbitration`. It keeps #898's scoping (request IDs for scoped queries, none for the overview) and queries up to `latest`. Logs are parsed with both `oracleAbi` and `sentinelOracleAbi`. `DisputeResolved.outcome` maps `RESOLVED_APPROVED` (3) to `secure: true` and `RESOLVED_DENIED` (4) to `secure: false`.
- `ProposalStatus` adds `ARBITRATING | SECURE | INSECURE | NO_RULING`. `deriveProposalStatus` maps an arbitration with no outcome to `ARBITRATING`, `ruled` to `SECURE` or `INSECURE`, and `outOfScope` or `timedOut` to `NO_RULING`. Verdict handling is unchanged. The lifecycle comment above `ProposalStatus` gains the new branch:

  ```text
  PROPOSED ──`DisputeTriggered`──> ARBITRATING ──`DisputeResolved`──> SECURE | INSECURE (final)
                                        └──`DisputeOutOfScope` / `ArbitrationTimedOut`──> NO_RULING (final)
  ```

`src/components/common/StatusBadge.tsx` adds the four cases. The badge ships in the same PR as the statuses, because its `default` branch would otherwise render them as `PROPOSED`.

### Arbitration details on the transaction page (Phase 3)

- `src/lib/oracle/abi.ts`: add `function ARBITRATOR() view returns (address)`.
- `loadArbitrator` in the oracle worker, and a `useOracleArbitrator(oracle, enabled)` hook with `staleTime: Infinity`, because `ARBITRATOR` is immutable. It is enabled only for a proposal with an arbitration.
- New `src/components/transaction/SafeTxArbitration.tsx`, rendered by `SafeTxProposal` under the oracle section when `proposal.arbitration !== null`. It shows the rows in the layout above, reusing `InlineBlockInfo`, `InlineExplorerTxLink` and `InlineAddress`. `(passed)` compares the deadline with the current block from `useConsensusState`. The reason is rendered as text with `break-all`.

### Tests

Same approach as #659 and #898: Vitest with a mocked viem client fed hand-built logs for the data layer, and React Testing Library with mocked hooks for components. No devnet or live-chain tests. Reviewers can still check by eye against the Aegis testnet (the current defaults), which has real `ARBITRATING` and `NO_RULING` proposals. No chain has a Council ruling or an out-of-scope record yet, so those cases rely on fixtures.

Phase 2, in `consensus.test.ts` (a `makeDisputeLog` builder beside `makeOracleResultLog`, served through `makeOracleAwareProvider`) and `StatusBadge.test.tsx`:

- `DisputeTriggered` alone gives `ARBITRATING`, and it stays `ARBITRATING` however far `toBlock` is past the proposal or the deadline.
- `DisputeResolved` gives `SECURE` for `RESOLVED_APPROVED` and `INSECURE` for `RESOLVED_DENIED`, with `context` carried through.
- `DisputeOutOfScope` gives `NO_RULING` with an `outOfScope` outcome and its `context`. `ArbitrationTimedOut` gives `NO_RULING` with a `timedOut` outcome.
- A dispute event for a different request is ignored.
- The oracle query sends all five selectors in `topics[0]` and `latest` as `toBlock`, and still filters by request ID only for scoped queries.
- A page with an explicit past `toBlock` still picks up a ruling emitted after it.
- The existing verdict cases pass unchanged.
- Each new status renders its label and variant class.

Phase 3, in `SafeTxProposals.test.tsx` and a small `loadArbitrator` test:

- No arbitration section for a proposal without an arbitration.
- A pending dispute shows the deadline, with `(passed)` only once the current block is past it.
- Each outcome shows its label and its block.
- The reason renders verbatim, including text that looks like markup, and an empty reason is hidden.
- The arbitrator's address is shown.

---

## Implementation Phases

### Phase 0: This plan

This document, as its own PR.

### Phase 1: Branding

One PR touching `SafenetLogo.tsx` (renamed), `Header.tsx`, `Header.test.tsx`, `routes/index.tsx`, and the deleted `SafenetBeta.svg`. Independent of the other phases.

### Phase 2: Arbitration statuses in the lists

One PR touching `lib/oracle/abi.ts`, `lib/consensus/transactions.ts`, `StatusBadge.tsx`, `consensus.test.ts` and `StatusBadge.test.tsx`. After it, the home and Safe lists show the arbitration statuses, and so does the `Status` row on the transaction page.

### Phase 3: Arbitration details on the transaction page

One PR, after Phase 2, touching `lib/oracle/abi.ts`, `lib/oracle/worker.ts`, a new arbitrator loader and hook, the new `SafeTxArbitration.tsx`, `SafeTxProposals.tsx` and their tests. If it grows past a comfortable review size, the arbitrator read (ABI, loader, worker, hook) splits off as its own PR first.

### Phase 4: Remove this plan

Delete this file once Phases 1 to 3 have shipped.

---

## Open Questions and Assumptions

### For the other authors

1. **What should a disputed proposal suggest?** The plan ends the arbitration section with: "Transactions that enter arbitration are never attested, whatever the ruling. To execute this one, propose it again in a later epoch or use the escape hatch." Should it offer both, one, or neither? Proposing again only works from the next epoch, because the request ID includes the epoch and the oracle rejects a duplicate, and the sentinels may split again. The escape hatch (the Guard's `announceTransaction`) needs no attestation, but waits out the announcement delay.
2. **Labels and colours of the new statuses.** `SECURE` in cyan like `APPROVED`, `NO_RULING` in the orange `warning` style, and short labels to fit the status column. Other choices are welcome.
3. **Waiting for the verdict is timed with `signingTimeout` (from #898).** The wait is measured from the proposal block (default 12 blocks). If production voting windows are longer than the setting, proposals show `TIMED_OUT` while the sentinels are still voting. Out of scope here, but the production default or the approach may need a look.
4. **Outcome query up to `latest`** (see Architecture Decision). This also applies to `OracleResult` on older Safe pages. Any objection?

### Pending design inputs

- A new `og-image.png` with AEGIS. The current image stays until then.
- Whether the favicon and app icon change. The Aegis logo uses the same mark, so they stay for now.
- Whether the Safe Green logo variant is meant for the header. The header keeps black on light and white on dark for now.

### Assumptions

- Only `SentinelOracle` is used from now on. The explorer's fallback for other oracles stays as it is (kept on purpose in #659).
- The arbitrator is shown under the label "Arbitrator" with its address. No label data file.
- No new settings fields, and no changes to current defaults.
- Council reasons and sentinel vote reasons stay raw strings. No Charter links or rule titles.
