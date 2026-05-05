# 1. OBJECTIVE

Add the final signature of the attestation to the info popup (InfoPopover) displayed in each proposal on the transaction details screen, once the signature becomes available (i.e., when signing is completed).

# 2. CONTEXT SUMMARY

The codebase is a SafeNet explorer application that displays transaction proposals and their attestation status. Key components:

1. **Transaction Proposals Display** (`SafeTxProposals.tsx`):
   - Shows a list of proposals for a transaction
   - Each proposal has an info button (InformationCircleIcon) that opens an InfoPopover
   - The InfoPopover currently shows: signature ID (sid) and Group ID (groupId)

2. **Attestation Status** (`signing.ts`):
   - `AttestationStatus` type contains: sid, groupId, sequence, lastUpdate, committed, signed, completed
   - When signing completes, the `SignCompleted` event includes a signature with fields `r` (point) and `z` (scalar)

3. **Signature Structure** (from ABI):
   - `SignCompleted` event: `((uint256 x, uint256 y) r, uint256 z) signature`
   - This is an ECDSA signature with:
     - `r`: x,y coordinates (point on curve)
     - `z`: signature value

# 3. APPROACH OVERVIEW

1. Extend the `AttestationStatus` type to include a signature field
2. Extract the signature from the `SignCompleted` event in the `loadLatestAttestationStatus` function
3. Display the signature in the InfoPopover when available
4. Use the existing `InlineHash` component to format the signature for display

**Why this approach:**
- Minimal changes to the data structure
- Reuses existing UI components (InlineHash, CopyButton)
- Follows the current pattern for displaying hashes in the InfoPopover
- Only shows signature when available (completed = true)

### Verification against group key:
The signature should be verifiable against the group's aggregated public key (groupId). The `groupId` field in `AttestationStatus` represents the group's aggregated public key. Verification would involve:
- Using the signature (r, z) to verify it was created using the group's aggregated key
- The `groupId` in the event corresponds to the aggregated public key of the signing group
- This provides cryptographic proof that the signature is valid for the group

# 4. IMPLEMENTATION STEPS

### Step 1: Update AttestationStatus Type
**File:** `/workspace/project/safenet/explorer/src/lib/coordinator/signing.ts`

**Goal:** Add signature field to the AttestationStatus type

**Method:** 
- Modify the `AttestationStatus` type definition to include a `signature` field
- Type: `Hex` (the signature can be represented as a hex string)

**Reference:** Lines 41-49

### Step 2: Extract Signature from SignCompleted Event
**File:** `/workspace/project/safenet/explorer/src/lib/coordinator/signing.ts`

**Goal:** Extract the signature when processing the `SignCompleted` event

**Method:**
- In the `aggregate` reduce function (lines 119-151), update the `SignCompleted` case to capture the signature
- When `SignCompleted` is encountered (line 142-144), store the signature
- In the final mapping (lines 153-177), include the signature in the returned object
- The signature should be formatted as a hex value combining r and z

**Reference:** Lines 119-177

### Step 3: Update ProposalInfoButton to Display Signature
**File:** `/workspace/project/safenet/explorer/src/components/transaction/SafeTxProposals.tsx`

**Goal:** Display the signature in the InfoPopover when available

**Method:**
- Update the InfoPopover content to include the signature
- Show signature only when `status.data.completed` is true
- Use `InlineHash` component to format the signature
- Add a `CopyButton` to copy the signature value
- Add a label "Signature:" to the display

**Reference:** Lines 35-59 (ProposalInfoButton function)

### Step 4: Update Tests
**File:** `/workspace/project/safenet/explorer/src/lib/coordinator/signing.test.ts`

**Goal:** Add test coverage for the signature extraction

**Method:**
- Add a test that verifies the signature is extracted from SignCompleted events
- Mock the signing events with a SignCompleted event containing a signature
- Assert that the returned AttestationStatus includes the signature

**Reference:** Line 1-114

# 5. TESTING AND VALIDATION

## Success Criteria:
1. The `AttestationStatus` type includes a `signature` field
2. When a `SignCompleted` event is present, the signature is correctly extracted and stored
3. The InfoPopover for a proposal displays the signature when signing is complete
4. The signature is formatted using `InlineHash` (truncated display)
5. The signature can be copied using the CopyButton
6. The `groupId` field provides the group's aggregated public key for signature verification
7. Users can verify the signature was created by the signing group using the groupId

## Manual Verification Steps:
1. Open a transaction details page with a proposal that has completed attestation
2. Click the info button (InformationCircleIcon) next to a proposal
3. Verify the signature is displayed in the format: `0x1234…5678`
4. Verify the CopyButton works to copy the full signature
5. Verify the groupId matches the group that attested this proposal
6. (Future) The signature and groupId can be used to verify the signature was created by the group

## Automated Tests:
- Unit tests in `signing.test.ts` verify signature extraction from SignCompleted events
- Existing tests in `SafeTxProposals.test.tsx` should pass with the updated data structure
