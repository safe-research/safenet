import { z } from "zod";
import { toPoint } from "../../frost/math.js";
import { checkedAddressSchema, hexBytes32Schema, hexDataSchema } from "../../types/schemas.js";

const eventBigIntSchema = z.coerce.bigint().nonnegative();

export const frostPointSchema = z
	.object({
		x: eventBigIntSchema,
		y: eventBigIntSchema,
	})
	.refine((point) => point.x !== 0n || point.y !== 0n)
	.transform((p) => toPoint(p));

export const frostCommitmentSchema = z.object({
	c: z.array(frostPointSchema),
	r: frostPointSchema,
	mu: eventBigIntSchema,
});

export const frostShareSchema = z.object({
	y: frostPointSchema,
	f: z.array(eventBigIntSchema),
});

export const keyGenEventSchema = z.object({
	gid: hexBytes32Schema,
	participants: hexBytes32Schema,
	count: z.int(),
	threshold: z.int(),
	context: hexBytes32Schema,
});

export const keyGenCommittedEventSchema = z.object({
	gid: hexBytes32Schema,
	identifier: eventBigIntSchema,
	commitment: frostCommitmentSchema,
	committed: z.boolean(),
});

export const keyGenSecretSharedEventSchema = z.object({
	gid: hexBytes32Schema,
	identifier: eventBigIntSchema,
	share: frostShareSchema,
	shared: z.boolean(),
});

export const keyGenConfirmedEventSchema = z.object({
	gid: hexBytes32Schema,
	identifier: eventBigIntSchema,
	confirmed: z.boolean(),
});

export const keyGenComplaintSubmittedEventSchema = z.object({
	gid: hexBytes32Schema,
	plaintiff: eventBigIntSchema,
	accused: eventBigIntSchema,
	compromised: z.boolean(),
});

export const keyGenComplaintRespondedEventSchema = z.object({
	gid: hexBytes32Schema,
	plaintiff: eventBigIntSchema,
	accused: eventBigIntSchema,
	secretShare: eventBigIntSchema,
});

export const nonceCommitmentsHashEventSchema = z.object({
	gid: hexBytes32Schema,
	identifier: eventBigIntSchema,
	chunk: eventBigIntSchema,
	commitment: hexBytes32Schema,
});

export const signRequestEventSchema = z.object({
	initiator: checkedAddressSchema,
	gid: hexBytes32Schema,
	message: hexBytes32Schema,
	sid: hexBytes32Schema,
	sequence: eventBigIntSchema,
});

export const nonceCommitmentsSchema = z.object({
	d: frostPointSchema,
	e: frostPointSchema,
});

export const nonceCommitmentsEventSchema = z.object({
	sid: hexBytes32Schema,
	identifier: eventBigIntSchema,
	nonces: nonceCommitmentsSchema,
});

export const signatureShareEventSchema = z.object({
	sid: hexBytes32Schema,
	identifier: eventBigIntSchema,
	z: eventBigIntSchema,
	root: hexBytes32Schema,
});

export const signatureSchema = z.object({
	z: eventBigIntSchema,
	r: frostPointSchema,
});

export const signedEventSchema = z.object({
	sid: hexBytes32Schema,
	signature: signatureSchema,
});

export const epochProposedEventSchema = z.object({
	activeEpoch: eventBigIntSchema,
	proposedEpoch: eventBigIntSchema,
	rolloverBlock: eventBigIntSchema,
	groupKey: frostPointSchema,
});

export const epochStagedEventSchema = z.object({
	activeEpoch: eventBigIntSchema,
	proposedEpoch: eventBigIntSchema,
	rolloverBlock: eventBigIntSchema,
	groupKey: frostPointSchema,
});

const transactionSchema = z.object({
	to: checkedAddressSchema,
	value: eventBigIntSchema,
	data: hexDataSchema,
	operation: z.union([z.literal(0), z.literal(1)]),
	nonce: eventBigIntSchema,
	chainId: eventBigIntSchema,
	account: checkedAddressSchema,
});

export const transactionProposedEventSchema = z.object({
	message: hexBytes32Schema,
	transactionHash: hexBytes32Schema,
	epoch: eventBigIntSchema,
	transaction: transactionSchema,
});

export const transactionAttestedEventSchema = z.object({
	message: hexBytes32Schema,
});

const baseEventTransitionParamsSchema = z.object({
	block: eventBigIntSchema,
	index: z.number(),
});

const keyGenEventTransitionSchema = baseEventTransitionParamsSchema.extend(keyGenEventSchema.shape).extend({
	id: z.literal("event_key_gen"),
});

const keyGenCommittedEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(keyGenCommittedEventSchema.shape)
	.extend({
		id: z.literal("event_key_gen_committed"),
	});

const keyGenSecretSharedEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(keyGenSecretSharedEventSchema.shape)
	.extend({
		id: z.literal("event_key_gen_secret_shared"),
	});

const keyGenComplaintSubmittedEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(keyGenComplaintSubmittedEventSchema.shape)
	.extend({
		id: z.literal("event_key_gen_complaint_submitted"),
	});

const keyGenComplaintRespondedEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(keyGenComplaintRespondedEventSchema.shape)
	.extend({
		id: z.literal("event_key_gen_complaint_responded"),
	});

const keyGenConfirmedEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(keyGenConfirmedEventSchema.shape)
	.extend({
		id: z.literal("event_key_gen_confirmed"),
	});

const nonceCommitmentsHashEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(nonceCommitmentsHashEventSchema.shape)
	.extend({
		id: z.literal("event_nonce_commitments_hash"),
	});

const signRequestEventTransitionSchema = baseEventTransitionParamsSchema.extend(signRequestEventSchema.shape).extend({
	id: z.literal("event_sign_request"),
});

const nonceCommitmentsEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(nonceCommitmentsEventSchema.shape)
	.extend({
		id: z.literal("event_nonce_commitments"),
	});

const signatureShareEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(signatureShareEventSchema.shape)
	.extend({
		id: z.literal("event_signature_share"),
	});

const signedEventTransitionSchema = baseEventTransitionParamsSchema.extend(signedEventSchema.shape).extend({
	id: z.literal("event_signed"),
});

const epochProposedEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(epochProposedEventSchema.shape)
	.extend({
		id: z.literal("event_epoch_proposed"),
	});

const epochStagedEventTransitionSchema = baseEventTransitionParamsSchema.extend(epochStagedEventSchema.shape).extend({
	id: z.literal("event_epoch_staged"),
});

const transactionProposedEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(transactionProposedEventSchema.shape)
	.extend({
		id: z.literal("event_transaction_proposed"),
	});

const transactionAttestedEventTransitionSchema = baseEventTransitionParamsSchema
	.extend(transactionAttestedEventSchema.shape)
	.extend({
		id: z.literal("event_transaction_attested"),
	});

const newBlockTransition = z.object({
	id: z.literal("block_new"),
	block: eventBigIntSchema,
});

export const stateTransitionSchema = z.discriminatedUnion("id", [
	// KeyGen Events
	keyGenEventTransitionSchema,
	keyGenCommittedEventTransitionSchema,
	keyGenSecretSharedEventTransitionSchema,
	keyGenComplaintSubmittedEventTransitionSchema,
	keyGenComplaintRespondedEventTransitionSchema,
	keyGenConfirmedEventTransitionSchema,
	nonceCommitmentsHashEventTransitionSchema,
	// Signing Events
	signRequestEventTransitionSchema,
	nonceCommitmentsEventTransitionSchema,
	signatureShareEventTransitionSchema,
	signedEventTransitionSchema,
	// Consensus Events
	epochProposedEventTransitionSchema,
	epochStagedEventTransitionSchema,
	transactionProposedEventTransitionSchema,
	transactionAttestedEventTransitionSchema,
	// Consensus Clock
	newBlockTransition,
]);
