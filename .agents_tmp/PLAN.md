# 1. OBJECTIVE

Add display of EIP-712 pre-image parts (domain hash and message hash) on the transaction details page in the explorer. The domain hash and message hash should be shown in an info pop-up triggered by an info icon next to the "SafeTxHash" label, similar to the existing proposal information pattern.

# 2. CONTEXT SUMMARY

The transaction details page is at `/safeTx` route and uses the following components:
- `SafeTxHeader` - displays Safe transaction header information including SafeTxHash
- `InfoPopover` - reusable pop-up component used elsewhere (e.g., in SafeTxProposals for signature ID and group ID)
- `calculateSafeTxHash` - existing function in `src/lib/safe/hashing.ts` that computes the final transaction hash using EIP-712

The SafeTxHash displayed is the EIP-712 message hash. The domain hash is the EIP-712 domain separator hash. Both are part of the EIP-712 typed data structure used for Safe transaction signing.

Key files identified:
- `src/routes/safeTx.tsx` - route component
- `src/components/transaction/SafeTxHeader.tsx` - displays SafeTxHash (needs modification)
- `src/components/common/InfoPopover.tsx` - reusable pop-up component
- `src/lib/safe/hashing.ts` - EIP-712 hashing utilities (needs extension)
- `src/lib/consensus/transactions.ts` - SafeTransaction type definition

# 3. APPROACH OVERVIEW

The "pre-image parts" refer to the intermediate EIP-712 hashes used in computing the final SafeTxHash:
- **Domain hash** = EIP-712 domain separator (hash of EIP712Domain type with chainId and verifyingContract)
- **Message hash** = hash of the typed SafeTx message (before combining with domain separator)
- **SafeTxHash** = final EIP-712 hash (combines domain separator + message hash with 0x1900 prefix)

Implementation approach:

1. **Extend hashing utilities** (`src/lib/safe/hashing.ts`): Add `calculateDomainHash` and `calculateMessageHash` functions to compute these intermediate EIP-712 hashes separately.

2. **Create SafeTxHashInfo component** (`src/components/transaction/SafeTxHashInfo.tsx`): A new component that:
   - Uses InfoPopover to show domain and message hashes
   - Displays an info icon next to SafeTxHash
   - Reuses existing components (InlineHash, CopyButton)

3. **Update SafeTxHeader** (`src/components/transaction/SafeTxHeader.tsx`): Integrate SafeTxHashInfo component to show the info icon and pop-up.

4. **Export new component**: Update exports as needed.

This approach reuses existing components and follows the established pattern from SafeTxProposals.tsx.

# 4. IMPLEMENTATION STEPS

## Step 1: Extend hashing utilities to compute domain and message hashes separately

**Goal**: Add `calculateDomainHash` and `calculateMessageHash` functions to `src/lib/safe/hashing.ts`

**Method**: 
- **Domain hash**: Compute EIP-712 domain separator using `hashTypedData` with:
  - Domain: `{ chainId, verifyingContract }`
  - Types: Empty object `{}` (only domain structure)
  - PrimaryType: Empty string `""` (no message)
  - Message: Empty object `{}`

- **Message hash**: Extract the current `calculateSafeTxHash` implementation to compute the typed message hash for SafeTx:
  - Domain: `{ chainId, verifyingContract }`
  - Types: `SafeTx` type definition with all fields
  - PrimaryType: `"SafeTx"`
  - Message: The transaction object

- **SafeTxHash**: Update to combine domain hash and message hash with EIP-712 prefix (0x1900)

**Technical note**: In EIP-712, the final hash is `keccak256(0x1900 + domainSeparator + messageHash)`. We need to compute these intermediate values.

**Reference**: `src/lib/safe/hashing.ts`

## Step 2: Create SafeTxHashInfo component

**Goal**: Create reusable component for displaying SafeTxHash with info pop-up

**Method**:
- Create `src/components/transaction/SafeTxHashInfo.tsx`
- Use InfoPopover with InformationCircleIcon as trigger
- Display domain hash and message hash in formatted form using InlineHash and CopyButton
- Export as named component

**Reference**: `src/components/common/InfoPopover.tsx`, `src/components/transaction/SafeTxProposals.tsx` (ProposalInfoButton pattern)

## Step 3: Update SafeTxHeader to use SafeTxHashInfo

**Goal**: Add info icon and pop-up next to SafeTxHash label

**Method**:
- Import SafeTxHashInfo component
- Wrap SafeTxHash display with SafeTxHashInfo
- Pass transaction prop to compute hashes
- Maintain existing layout and functionality

**Reference**: `src/components/transaction/SafeTxHeader.tsx`

## Step 4: Update exports if needed

**Goal**: Ensure new component is properly exported

**Method**:
- Check if there's an index file for transaction components
- Add export for SafeTxHashInfo if needed

**Reference**: Check for existing exports in transaction directory

## Step 5: Update tests

**Goal**: Verify the new functionality works correctly

**Method**:
- Add test for SafeTxHashInfo component
- Update SafeTxHeader tests to verify info icon is present
- Verify hashes are displayed correctly

**Reference**: `src/components/transaction/SafeTxHeader.test.tsx`

# 5. TESTING AND VALIDATION

## Expected behaviors:
1. Info icon (InformationCircleIcon) appears next to "SafeTxHash" label
2. Clicking info icon shows pop-up with domain hash and message hash
3. Pop-up closes when clicking icon again or clicking outside
4. Hashes are displayed in formatted form (truncated) with copy button
5. All existing functionality in SafeTxHeader remains intact
6. SafeTxHashInfo can be reused in other contexts if needed

## Validation criteria:
- Component renders without errors
- Info icon is visible
- Pop-up shows correct domain and message hashes
- Hashes match the expected values from EIP-712 structure
- Existing tests pass
- New tests added and passing
