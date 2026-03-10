# Feature Proposal: Explorer Test Coverage & Refactoring
Component: `explorer`

---

## Overview

The explorer workspace has significant gaps in test coverage — the majority of components, all hooks, several library modules, and all route files lack unit tests. This feature addresses the most impactful coverage gaps, extracts reusable patterns, and adds inline comments for non-obvious data transformations.

**Phases:**

1. **Phase 1** — Library module tests (`consensus.ts`, `packets.ts`, `schemas.ts`, `chains.ts`, `settings.ts`)
2. **Phase 2** — Hook tests (all `useX` hooks)
3. **Phase 3** — Component refactoring and tests

---

## Architecture Decision

No new components or architectural changes. The focus is on:

- Adding tests for existing untested code using Vitest and `@testing-library/react` (already available in the project)
- Improving code organization where shared patterns can be extracted
- Adding inline documentation for data transformations and contract interaction logic

### Alternatives Considered

- **End-to-end tests with Playwright/Cypress**: These would test the full user flow but are heavier and slower. Unit and component tests provide faster feedback for the identified gaps. E2E tests can be considered in a separate initiative.
- **Snapshot tests for components**: Snapshot tests are brittle and don't validate behavior. Component tests with `@testing-library/react` are preferred.

---

## Tech Specs

### Missing Test Coverage

#### Library Modules (no tests or partial coverage)

| Module | What needs testing |
|---|---|
| `lib/consensus.ts` | `loadConsensusState`, `loadTransactionProposals`, `loadProposedSafeTransaction`, `loadEpochsState`, `loadEpochRolloverHistory`, `postTransactionProposal` — all untested. These are the core data-fetching functions. |
| `lib/packets.ts` | Packet construction logic — untested |
| `lib/schemas.ts` | Schema validation (`bigIntSchema`, `checkedAddressSchema`, `hexDataSchema`) — untested |
| `lib/chains.ts` | Chain configuration — untested |
| `lib/settings.ts` | Settings persistence (localStorage read/write with error handling) — untested. Contains `console.error` calls that silently swallow parse failures. |
| `lib/safe/service.ts` | Safe Transaction Service API interaction — untested |
| `lib/safe/formatting.ts` | Transaction formatting utilities — untested |
| `lib/validators/info.ts` | Validator info fetching — untested |

#### Hooks (none have tests)

| Hook | What needs testing |
|---|---|
| `useConsensusState` | Correct query key, refetch interval behavior, initial data |
| `useRecentTransactionProposals` | Pagination logic (`itemsToShow` increment), data transformation |
| `useEpochRolloverHistory` | Infinite query pagination, genesis detection |
| `useSettings` | Settings read/write, default value handling |
| `useProvider` | Provider creation from settings |
| `useProposalsForTransaction` | Transaction proposal filtering |
| `useSubmitProposal` | Mutation behavior, error handling |
| `useKeyGenDetails` | KeyGen data transformation |
| `useSigningProgress` | Signing progress computation |
| `useValidatorInfo` | Validator data fetching |
| `useEpochsState` | Epoch state query |
| `useSafeTransactionDetails` | Transaction detail fetching |

#### Components (none have tests except `ConsensusSettingsForm` and `TransactionProposalsList`)

| Component | Priority | What needs testing |
|---|---|---|
| `SearchBar` | High | Input validation, navigation behavior (address vs hash detection) |
| `SafeTxOverview` | High | Correct rendering of transaction data, attestation status |
| `SafeTxDataDetails` | Medium | Data display formatting, edge cases (empty data, large data) |
| `EpochCard` | Medium | Epoch state rendering, group ID display |
| `ErrorBoundary` | Medium | Error catching, fallback UI rendering |
| `InlineAddress` | Low | Address truncation, copy behavior |

### Security Observations

| Location | Issue | Severity | Action |
|---|---|---|---|
| `components/transaction/SafeTxDataDetails.tsx:17` | `href={settings.decoder}{data}` — `settings.decoder` is user-configurable via localStorage. A malicious `javascript:` URL would execute in the user's browser. | Medium | Validate the decoder URL uses `https://` protocol before rendering as a link |
| `lib/safe/service.ts` | Calls external Safe API endpoint without timeout configuration | Low | Add request timeout and consider rate limiting |
| `lib/settings.ts` | Sensitive settings (RPC endpoint URLs) stored in localStorage in plain text | Info | Document the trust model: explorer is a client-side tool and localStorage is trusted |

### Code Smells and Improvements

| Location | Issue | Action |
|---|---|---|
| `lib/settings.ts:37,62,84` | `console.error` used for settings parse failures — these are silently swallowed | Add structured error reporting or at minimum document why silent failure is acceptable (graceful degradation to defaults) |
| `lib/settings.ts` | `loadSafeApiSettings()` and `updateSafeApiSettings()` functions and the `SafeApiSettings` type appear unused in the codebase | Confirm dead code and remove, or document planned future use |
| `lib/consensus.ts:148-149` | `attestationKey` function is defined inline — used for matching `TransactionProposed` to `TransactionAttested` events | Extract and document the epoch-scoped matching logic (a transaction can be attested in different epochs after re-proposal) |
| `lib/consensus.ts:79-86` | `transactionEventSelectors` array construction | Add inline comment explaining why both event selectors are in the first topic position (OR-filter on event signature in `eth_getLogs`) |
| `hooks/useRecentTransactionProposals.tsx` | The `itemsToShow` state is incremented by a hardcoded `5` | Extract to a named constant |
| `reportWebVitals.ts` | Appears to be leftover CRA boilerplate | Evaluate if web vitals reporting is used; if not, remove |
| `components/epoch/EpochCard.tsx` | Large component handling multiple epoch states | Consider splitting into sub-components per state (KeyGen, Signing, Idle) |
| `components/epoch/KeyGenStatusItem.tsx:6-19` | `statusLabel()` and `statusColor()` are hardcoded in the component | Extract to `lib/` as reusable domain-logic utilities |
| `components/search/SearchBar.tsx:50` | Input accepts any text and navigates without validating it is a valid hex hash | Add input validation before navigation |
| `main.tsx:36` | `if (rootElement && !rootElement.innerHTML)` — checking `innerHTML` is unusual for determining mount readiness | Use a data attribute or dedicated check |

### Refactoring Opportunities

| Location | Issue | Action |
|---|---|---|
| Settings forms | `ConsensusSettingsForm.tsx` and `UiSettingsForm.tsx` have identical form patterns (useForm setup, error state, submit handling) | Extract a shared form wrapper component |
| Validator list rendering | `KeyGenStatusItem.tsx` (lines 48-54) and `SafeTxAttestationStatus.tsx` (lines 27-34) repeat the same `ValidatorList` rendering pattern | Abstract into a shared helper |
| `lib/coordinator/signing.ts` | `loadLatestAttestationStatus` is 182 lines of log aggregation | Split into `filterSigningEvents()`, `parseProgressEvents()`, `aggregateEventLogs()`, `computeAttestationStatus()` |
| Hook boilerplate | All hooks in `/hooks/` follow the same pattern: useQuery → useSettings → useProvider → compose | Consider a factory function to reduce repetitive setup |

### Accessibility Issues

| Location | Issue | Action |
|---|---|---|
| `components/search/SearchBar.tsx` | Select and input fields lack ARIA labels; magnifying glass icon button has no accessible name | Add `aria-label` attributes |
| `components/epoch/EpochRolloverItem.tsx` | Expand/collapse button state is not announced to screen readers | Add `aria-expanded` attribute |

### Inline Comments Needed

| Location | What to document |
|---|---|
| `lib/consensus.ts:126-180` | The `loadTransactionProposals` function uses raw `eth_getLogs` instead of viem's typed `getLogs` — document why (OR-filtering on multiple event signatures via topic arrays) |
| `lib/safe/hashing.ts` | Document that this SafeTx hash computation must match the Safe contract's EIP-712 implementation exactly, and reference the `SafeTransaction.sol` library |
| `lib/coordinator/signing.ts` | Document the relationship between FROST signing ceremonies, sequence numbers, and nonce chunks |
| `lib/coordinator/keygen.ts` | Document the three rounds of KeyGen and which contract events correspond to each round |
| `lib/coordinator/keygen.ts:112-127` | `computeStartBlock()` has three different fallback strategies (blocksPerEpoch, prevStagedAt, maxBlockRange) — document the priority and rationale for each |
| `hooks/useSafeTransactionDetails.tsx:8-27` | `findAny()` uses `Promise.any()` with a pattern that throws "not found" errors — document why this approach works better than sequential attempts |

---

## Implementation Phases

### Phase 1 — Library Module Tests (independent PR)

**Scope:** Add tests for core library modules that the rest of the explorer depends on.

**Files touched:**
- `explorer/src/lib/consensus.test.ts` — extend existing tests with `loadConsensusState`, `loadTransactionProposals`, `postTransactionProposal`
- `explorer/src/lib/schemas.test.ts` — new
- `explorer/src/lib/chains.test.ts` — new
- `explorer/src/lib/settings.test.ts` — new
- `explorer/src/lib/safe/formatting.test.ts` — new
- `explorer/src/lib/packets.test.ts` — new
- `explorer/src/lib/consensus.ts` — add inline comments

**Test cases:**
- `bigIntSchema` — parsing valid/invalid bigint strings
- `checkedAddressSchema` — valid checksummed, lowercase, invalid addresses
- `loadTransactionProposals` — mock provider, verify event parsing, attestation matching
- `postTransactionProposal` — mock fetch, verify request body serialization
- Settings — roundtrip read/write, corrupt localStorage graceful fallback

---

### Phase 2 — Hook Tests (independent PR)

**Scope:** Add tests for all React hooks using Vitest and TanStack Query test utilities.

**Files touched:**
- `explorer/src/hooks/useConsensusState.test.tsx` — new
- `explorer/src/hooks/useSettings.test.tsx` — new
- `explorer/src/hooks/useRecentTransactionProposals.test.tsx` — new
- `explorer/src/hooks/useEpochRolloverHistory.test.tsx` — new
- `explorer/src/hooks/useProvider.test.tsx` — new
- `explorer/src/hooks/useSubmitProposal.test.tsx` — new

**Test approach:** Wrap hooks in a test `QueryClientProvider` and use `renderHook` from `@testing-library/react`. Mock the provider and settings context.

---

### Phase 3 — Component Tests and Refactoring (independent PR)

**Scope:** Add component tests for the highest-priority untested components and perform minor refactoring.

**Files touched:**
- `explorer/src/components/search/SearchBar.test.tsx` — new
- `explorer/src/components/transaction/SafeTxOverview.test.tsx` — new
- `explorer/src/components/ErrorBoundary.test.tsx` — new
- `explorer/src/components/common/InlineAddress.test.tsx` — new
- `explorer/src/reportWebVitals.ts` — evaluate and potentially remove

---

## Open Questions / Assumptions

1. **Test infrastructure**: The project already has `vitest` and `@testing-library/react` configured. Confirm that `jsdom` or `happy-dom` is set up as the test environment for component tests (check `vitest.config.ts`).
2. **Mock provider strategy**: Hook tests need a mock `PublicClient`. Should this be a shared test utility (e.g., `explorer/src/__tests__/mockProvider.ts`) or inline per test?
3. **`reportWebVitals.ts`**: Is web vitals reporting actively used? If not, removing it simplifies the codebase. If yes, it should be tested.
4. **Settings error handling**: The current `console.error` approach for corrupt settings data provides graceful degradation. Should this be upgraded to structured error reporting (e.g., via a logging utility) or is silent fallback acceptable?
5. **Dead code in settings**: Are `loadSafeApiSettings()`, `updateSafeApiSettings()`, and the `SafeApiSettings` type planned for future use, or should they be removed?
6. **Decoder URL validation**: The user-configurable decoder URL is used in an `<a href>`. Should we restrict it to `https://` only, or allow other protocols? This has security implications (see Security Observations).
7. **Accessibility standards**: Should the explorer target WCAG 2.1 AA compliance? If so, the ARIA and accessibility fixes should be prioritized higher.
