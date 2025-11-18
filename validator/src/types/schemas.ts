import { type Address, getAddress, type Hex, isAddress, isHex } from "viem";
import { z } from "zod";

export const checkedAddressSchema = z
	.string()
	// Strict is disabled here, as a manual check for the correct checksum is performed.
	// Viem always allows to get around the checksum when providing an all lowercase address.
	// With strict to `false` and the manual check the additional overhead is minimal as the
	// result of `isAddress` is cached internaly in Viem (strict is part of the cache key).
	.refine(
		(arg) => isAddress(arg, { strict: false }) && arg === getAddress(arg),
		"Invalid address format or checksum",
	)
	.transform((arg) => arg as Address);

export const hexDataSchema = z
	.string()
	.refine(isHex, "Value is not a valid hex string")
	.transform((val) => val as Hex);

export const validatorConfigSchema = z.object({
	RPC_URL: z.url(),
	CONSENSUS_CORE_ADDRESS: checkedAddressSchema,
});

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
});

export const keyGenCommittedEventSchema = z.object({
	gid: hexDataSchema,
	identifier: z.bigint().positive(),
	commitment: frostCommitmentSchema,
});

export const keyGenSecretSharedEventSchema = z.object({
	gid: hexDataSchema,
	identifier: z.bigint().positive(),
	share: frostShareSchema,
});

export const nonceCommitmentsHashEventSchema = z.object({
	gid: hexDataSchema,
	identifier: z.bigint().positive(),
	chunk: z.bigint().nonnegative(),
	commitment: hexDataSchema,
});

export const signRequestEventSchema = z.object({
	gid: hexDataSchema,
	sid: hexDataSchema,
	message: hexDataSchema,
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

export const epochProposedEventSchema = z.object({
	activeEpoch: z.bigint().positive(),
	proposedEpoch: z.bigint().positive(),
	timestamp: z.bigint().positive(),
	groupKey: frostPointSchema,
});

