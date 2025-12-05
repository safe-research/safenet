import { z } from "zod";
import { toPoint } from "../../frost/math.js";
import type { GroupId, ParticipantId, SignatureId } from "../../frost/types.js";
import { hexDataSchema } from "../../types/schemas.js";
import { SqliteQueue } from "../../utils/queue.js";
import type { ActionWithRetry } from "./types.js";

const groupIdSchema = hexDataSchema.transform((v) => v as GroupId);
const coercedBigIntSchema = z.coerce.bigint().nonnegative();
const participantIdSchema = coercedBigIntSchema.transform((v) => v as ParticipantId);
const signatureIdSchema = hexDataSchema.transform((v) => v as SignatureId);

// Complex objects from imports
const frostPointSchema = z
	.object({
		x: coercedBigIntSchema,
		y: coercedBigIntSchema,
	})
	.transform((p) => toPoint(p));

const proofOfKnowledgeSchema = z.object({
	r: frostPointSchema,
	mu: coercedBigIntSchema,
});

const proofOfAttestationParticipationSchema = z.array(hexDataSchema);

const publicNonceCommitmentsSchema = z.object({
	hidingNonceCommitment: frostPointSchema,
	bindingNonceCommitment: frostPointSchema,
});
// --- Signing Actions ---

const requestSignatureSchema = z.object({
	id: z.literal("sign_request"),
	groupId: groupIdSchema,
	message: hexDataSchema,
});

const registerNonceCommitmentsSchema = z.object({
	id: z.literal("sign_register_nonce_commitments"),
	groupId: groupIdSchema,
	nonceCommitmentsHash: hexDataSchema,
});

const revealNonceCommitmentsSchema = z.object({
	id: z.literal("sign_reveal_nonce_commitments"),
	signatureId: signatureIdSchema,
	nonceCommitments: publicNonceCommitmentsSchema,
	nonceProof: z.array(hexDataSchema),
});

const publishSignatureShareSchema = z.object({
	id: z.literal("sign_publish_signature_share"),
	signatureId: signatureIdSchema,
	signersRoot: hexDataSchema,
	signersProof: z.array(hexDataSchema),
	groupCommitment: frostPointSchema,
	commitmentShare: frostPointSchema,
	signatureShare: coercedBigIntSchema,
	lagrangeCoefficient: coercedBigIntSchema,
	callbackContext: hexDataSchema.optional(),
});

const signingActionSchema = z.discriminatedUnion("id", [
	requestSignatureSchema,
	registerNonceCommitmentsSchema,
	revealNonceCommitmentsSchema,
	publishSignatureShareSchema,
]);

// --- KeyGen Actions ---

export const startKeyGenSchema = z.object({
	id: z.literal("key_gen_start"),
	participants: hexDataSchema,
	count: coercedBigIntSchema,
	threshold: coercedBigIntSchema,
	context: hexDataSchema,
	participantId: participantIdSchema,
	commitments: z.array(frostPointSchema),
	pok: proofOfKnowledgeSchema,
	poap: proofOfAttestationParticipationSchema,
});

export const publishSecretSharesSchema = z.object({
	id: z.literal("key_gen_publish_secret_shares"),
	groupId: groupIdSchema,
	verificationShare: frostPointSchema,
	shares: z.array(coercedBigIntSchema),
});

export const keyGenCofirmSchema = z.object({
	id: z.literal("key_gen_confirm"),
	groupId: groupIdSchema,
	callbackContext: hexDataSchema.optional(),
});

export const keyGenActionSchema = z.discriminatedUnion("id", [
	startKeyGenSchema,
	publishSecretSharesSchema,
	keyGenCofirmSchema,
]);

// --- Consensus Actions ---

export const attestTransactionSchema = z.object({
	id: z.literal("consensus_attest_transaction"),
	epoch: coercedBigIntSchema,
	transactionHash: hexDataSchema,
	signatureId: signatureIdSchema,
});

export const stageEpochSchema = z.object({
	id: z.literal("consensus_stage_epoch"),
	proposedEpoch: coercedBigIntSchema,
	rolloverBlock: coercedBigIntSchema,
	groupId: groupIdSchema,
	signatureId: signatureIdSchema,
});

export const consensusActionSchema = z.discriminatedUnion("id", [attestTransactionSchema, stageEpochSchema]);

// --- Protocol Action & Retry ---

const protocolActionSchema = z.union([signingActionSchema, keyGenActionSchema, consensusActionSchema]);

const actionWithRetrySchema = z.intersection(
	protocolActionSchema,
	z.object({
		retryCount: z.number(),
	}),
);

export class SqliteActionQueue extends SqliteQueue<ActionWithRetry> {
	constructor(path: string) {
		super(actionWithRetrySchema, path, "actions");
	}
}
