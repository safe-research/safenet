import { z } from "zod";
import { safeTransactionSchema } from "../consensus/verify/safeTx/schemas.js";
import { checkedAddressSchema, hexDataSchema } from "../types/schemas.js";

export const frostPointSchema = z.object({
	x: z.bigint().nonnegative(),
	y: z.bigint().nonnegative(),
});

export const frostCommitmentSchema = z.object({
	c: z.array(frostPointSchema),
	r: frostPointSchema,
	mu: z.bigint().nonnegative(),
});

export const frostShareSchema = z.object({
	y: frostPointSchema,
	f: z.array(z.bigint().nonnegative()),
});

export const keyGenEventSchema = z.object({
	gid: hexDataSchema,
	participants: hexDataSchema,
	count: z.bigint().positive(),
	threshold: z.bigint().positive(),
	context: hexDataSchema,
});

export const keyGenCommittedEventSchema = z.object({
	gid: hexDataSchema,
	identifier: z.bigint().positive(),
	commitment: frostCommitmentSchema,
	committed: z.boolean(),
});

export const keyGenSecretSharedEventSchema = z.object({
	gid: hexDataSchema,
	identifier: z.bigint().positive(),
	share: frostShareSchema,
	completed: z.boolean(),
});

export const nonceCommitmentsHashEventSchema = z.object({
	gid: hexDataSchema,
	identifier: z.bigint().positive(),
	chunk: z.bigint().nonnegative(),
	commitment: hexDataSchema,
});

export const signRequestEventSchema = z.object({
	initiator: checkedAddressSchema,
	gid: hexDataSchema,
	message: hexDataSchema,
	sid: hexDataSchema,
	sequence: z.bigint().nonnegative(),
});

export const nonceCommitmentsSchema = z.object({
	d: frostPointSchema,
	e: frostPointSchema,
});

export const nonceCommitmentsEventSchema = z.object({
	sid: hexDataSchema,
	identifier: z.bigint().positive(),
	nonces: nonceCommitmentsSchema,
});

export const signatureShareEventSchema = z.object({
	sid: hexDataSchema,
	identifier: z.bigint().positive(),
	z: z.bigint().nonnegative(),
});

export const signatureSchema = z.object({
	z: z.bigint().positive(),
	r: frostPointSchema,
});

export const signedEventSchema = z.object({
	sid: hexDataSchema,
	signature: signatureSchema,
});

export const epochProposedEventSchema = z.object({
	activeEpoch: z.bigint().nonnegative(),
	proposedEpoch: z.bigint().positive(),
	rolloverBlock: z.bigint().positive(),
	groupKey: frostPointSchema,
});

export const epochStagedEventSchema = z.object({
	activeEpoch: z.bigint().nonnegative(),
	proposedEpoch: z.bigint().positive(),
	rolloverBlock: z.bigint().positive(),
	groupKey: frostPointSchema,
});

export const transactionProposedEventSchema = z.object({
	message: hexDataSchema,
	transactionHash: hexDataSchema,
	epoch: z.bigint().nonnegative(),
	transaction: safeTransactionSchema,
});

export const transactionAttestedEventSchema = z.object({
	message: hexDataSchema,
});
