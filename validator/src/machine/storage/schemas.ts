import { z } from "zod";
import { epochRolloverPacketSchema } from "../../consensus/verify/rollover/schemas.js";
import { safeTransactionPacketSchema } from "../../consensus/verify/safeTx/schemas.js";
import type { GroupId, ParticipantId, SignatureId } from "../../frost/types.js";
import { hexBytes32Schema } from "../../types/schemas.js";

// --- Base Type Definitions (for Zod) ---

const groupIdSchema = hexBytes32Schema.transform((v) => v as GroupId);
const coercedBigIntSchema = z.coerce.bigint().nonnegative();
const participantIdSchema = coercedBigIntSchema.transform((v) => v as ParticipantId);
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
			nonce: coercedBigIntSchema,
			value: coercedBigIntSchema,
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
		signatureId: signatureIdSchema,
		stateJson: z.string(),
	}),
);

// --- 1. RolloverState Schemas ---

const waitingForRolloverSchema = z.object({
	id: z.literal("waiting_for_rollover"),
});

const collectingCommitmentsSchema = z.object({
	id: z.literal("collecting_commitments"),
	groupId: groupIdSchema,
	nextEpoch: coercedBigIntSchema,
	deadline: coercedBigIntSchema,
});

const complaintsDataSchema = z.object({
	unresponded: coercedBigIntSchema,
	total: coercedBigIntSchema,
});

const complaintsSchema = z.record(z.string(), complaintsDataSchema);

const collectingSharesSchema = z.object({
	id: z.literal("collecting_shares"),
	groupId: groupIdSchema,
	nextEpoch: coercedBigIntSchema,
	deadline: coercedBigIntSchema,
	complaints: complaintsSchema,
	lastParticipant: participantIdSchema.optional(),
});

const collectingConfirmationsSchema = z.object({
	id: z.literal("collecting_confirmations"),
	groupId: groupIdSchema,
	nextEpoch: coercedBigIntSchema,
	complaints: complaintsSchema,
	complaintDeadline: coercedBigIntSchema,
	responseDeadline: coercedBigIntSchema,
	deadline: coercedBigIntSchema,
	lastParticipant: participantIdSchema.optional(),
	confirmationsFrom: participantIdSchema.array(),
});

const signRolloverSchema = z.object({
	id: z.literal("sign_rollover"),
	groupId: groupIdSchema,
	nextEpoch: coercedBigIntSchema,
	message: hexBytes32Schema,
	responsible: participantIdSchema,
});

export const rolloverStateSchema = z.discriminatedUnion("id", [
	waitingForRolloverSchema,
	collectingCommitmentsSchema,
	collectingSharesSchema,
	collectingConfirmationsSchema,
	signRolloverSchema,
]);

// --- 2. SigningState Schemas ---

const baseSigningStateSchema = z.object({
	packet: packetSchema,
});

const waitingForRequestSchema = z.object({
	id: z.literal("waiting_for_request"),
	responsible: participantIdSchema.optional(),
	signers: z.array(participantIdSchema),
	deadline: coercedBigIntSchema,
});

const collectNonceCommitmentsSchema = z.object({
	id: z.literal("collect_nonce_commitments"),
	signatureId: signatureIdSchema,
	lastSigner: participantIdSchema.optional(),
	deadline: coercedBigIntSchema,
});

const collectSigningSharesSchema = z.object({
	id: z.literal("collect_signing_shares"),
	signatureId: signatureIdSchema,
	sharesFrom: z.array(participantIdSchema),
	lastSigner: participantIdSchema.optional(),
	deadline: coercedBigIntSchema,
});

const waitingForAttestationSchema = z.object({
	id: z.literal("waiting_for_attestation"),
	signatureId: signatureIdSchema,
	responsible: participantIdSchema.optional(),
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

const groupInfoSchema = z.object({
	groupId: groupIdSchema,
	participantId: participantIdSchema,
});

export const consensusStateSchema = z.object({
	genesisGroupId: groupIdSchema.optional(),
	activeEpoch: coercedBigIntSchema,
	stagedEpoch: coercedBigIntSchema,
	groupPendingNonces: z.record(groupIdSchema, z.boolean()),
	epochGroups: z.record(z.string(), groupInfoSchema),
	signatureIdToMessage: z.record(signatureIdSchema, hexBytes32Schema),
});
