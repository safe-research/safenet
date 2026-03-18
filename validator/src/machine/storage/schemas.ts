import { z } from "zod";
import { epochRolloverPacketSchema } from "../../consensus/verify/rollover/schemas.js";
import { safeTransactionPacketSchema } from "../../consensus/verify/safeTx/schemas.js";
import type { GroupId, SignatureId } from "../../frost/types.js";
import { checkedAddressSchema, hexBytes32Schema } from "../../types/schemas.js";

// --- Base Type Definitions (for Zod) ---

const groupIdSchema = hexBytes32Schema.transform((v) => v as GroupId);
const coercedBigIntSchema = z.coerce.bigint().nonnegative();
const signatureIdSchema = hexBytes32Schema.transform((v) => v as SignatureId);

// Overwrite bigint fields to accept strings
const dbSafeTransactionPacketSchema = safeTransactionPacketSchema.extend({
	domain: safeTransactionPacketSchema.shape.domain.extend({
		chain: coercedBigIntSchema,
	}),
	proposal: safeTransactionPacketSchema.shape.proposal.extend({
		epoch: coercedBigIntSchema,
		transaction: safeTransactionPacketSchema.shape.proposal.shape.transaction.extend({
			chainId: coercedBigIntSchema,
			value: coercedBigIntSchema,
			safeTxGas: coercedBigIntSchema,
			baseGas: coercedBigIntSchema,
			gasPrice: coercedBigIntSchema,
			nonce: coercedBigIntSchema,
		}),
	}),
});
const dbEpochRolloverPacketSchema = epochRolloverPacketSchema.extend({
	domain: epochRolloverPacketSchema.shape.domain.extend({
		chain: coercedBigIntSchema,
	}),
	rollover: epochRolloverPacketSchema.shape.rollover.extend({
		activeEpoch: coercedBigIntSchema,
		proposedEpoch: coercedBigIntSchema,
		rolloverBlock: coercedBigIntSchema,
		groupKeyX: coercedBigIntSchema,
		groupKeyY: coercedBigIntSchema,
	}),
});

const packetSchema = z.union([dbSafeTransactionPacketSchema, dbEpochRolloverPacketSchema]);

// --- SQLite Base Query Schemas ---

export const jsonQueryResultSchema = z
	.object({
		stateJson: z.string(),
	})
	.optional();

export const signingQueryResultSchema = z.array(
	z.object({
		id: hexBytes32Schema,
		stateJson: z.string(),
	}),
);

// --- 1. RolloverState Schemas ---

const waitingForGenesisSchema = z.object({
	id: z.literal("waiting_for_genesis"),
});

const skipGenesisSchema = z.object({
	id: z.literal("skip_genesis"),
});

const skipEpochSchema = z.object({
	id: z.literal("epoch_skipped"),
	nextEpoch: coercedBigIntSchema,
});

const collectingCommitmentsSchema = z.object({
	id: z.literal("collecting_commitments"),
	groupId: groupIdSchema,
	nextEpoch: coercedBigIntSchema,
	deadline: coercedBigIntSchema,
});

const complaintsDataSchema = z.object({
	unresponded: z.number(),
	total: z.number(),
});

const complaintsSchema = z.record(z.string(), complaintsDataSchema);

const collectingSharesSchema = z.object({
	id: z.literal("collecting_shares"),
	groupId: groupIdSchema,
	nextEpoch: coercedBigIntSchema,
	deadline: coercedBigIntSchema,
	complaints: complaintsSchema,
	sharesFrom: checkedAddressSchema.array(),
	lastParticipant: checkedAddressSchema.optional(),
});

const collectingConfirmationsSchema = z.object({
	id: z.literal("collecting_confirmations"),
	groupId: groupIdSchema,
	nextEpoch: coercedBigIntSchema,
	complaints: complaintsSchema,
	complaintDeadline: coercedBigIntSchema,
	responseDeadline: coercedBigIntSchema,
	deadline: coercedBigIntSchema,
	lastParticipant: checkedAddressSchema.optional(),
	sharesFrom: checkedAddressSchema.array(),
	confirmationsFrom: checkedAddressSchema.array(),
});

const signRolloverSchema = z.object({
	id: z.literal("sign_rollover"),
	groupId: groupIdSchema,
	nextEpoch: coercedBigIntSchema,
	message: hexBytes32Schema,
});

const epochStagedSchema = z.object({
	id: z.literal("epoch_staged"),
	nextEpoch: coercedBigIntSchema,
});

export const rolloverStateSchema = z.discriminatedUnion("id", [
	waitingForGenesisSchema,
	skipGenesisSchema,
	skipEpochSchema,
	collectingCommitmentsSchema,
	collectingSharesSchema,
	collectingConfirmationsSchema,
	signRolloverSchema,
	epochStagedSchema,
]);

// --- 2. SigningState Schemas ---

const baseSigningStateSchema = z.object({
	packet: packetSchema,
});

const waitingForRequestSchema = z.object({
	id: z.literal("waiting_for_request"),
	responsible: checkedAddressSchema.optional(),
	signers: z.array(checkedAddressSchema),
	deadline: coercedBigIntSchema,
});

const collectNonceCommitmentsSchema = z.object({
	id: z.literal("collect_nonce_commitments"),
	signatureId: signatureIdSchema,
	lastSigner: checkedAddressSchema.optional(),
	deadline: coercedBigIntSchema,
});

const collectSigningSharesSchema = z.object({
	id: z.literal("collect_signing_shares"),
	signatureId: signatureIdSchema,
	sharesFrom: z.array(checkedAddressSchema),
	lastSigner: checkedAddressSchema.optional(),
	deadline: coercedBigIntSchema,
});

const waitingForAttestationSchema = z.object({
	id: z.literal("waiting_for_attestation"),
	signatureId: signatureIdSchema,
	responsible: checkedAddressSchema.optional(),
	deadline: coercedBigIntSchema,
});

export const signingStateSchema = z.intersection(
	baseSigningStateSchema,
	z.discriminatedUnion("id", [
		waitingForRequestSchema,
		collectNonceCommitmentsSchema,
		collectSigningSharesSchema,
		waitingForAttestationSchema,
	]),
);

// --- 3. MutableConsensusState Schema ---

export const consensusStateSchema = z.object({
	genesisGroupId: groupIdSchema.optional(),
	activeEpoch: coercedBigIntSchema,
	groupPendingNonces: z.record(groupIdSchema, z.boolean()),
	epochGroups: z.record(z.string(), groupIdSchema),
	signatureIdToMessage: z.record(signatureIdSchema, hexBytes32Schema),
});
