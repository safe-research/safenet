import z from "zod";
import { checkedAddressSchema, hexDataSchema } from "../../../types/schemas.js";

const safeMetaTransactionSchema = z.object({
	to: checkedAddressSchema,
	value: z.bigint().nonnegative(),
	data: hexDataSchema,
	operation: z.union([z.literal(0), z.literal(1)]),
});

const partialSafeTransactionSchema = safeMetaTransactionSchema.extend({
	nonce: z.bigint().nonnegative(),
});

export const safeTransactionSchema = partialSafeTransactionSchema.extend({
	chainId: z.bigint().nonnegative(),
	account: checkedAddressSchema,
});

const transactionProposalSchema = z.object({
	epoch: z.bigint().nonnegative(),
	transaction: safeTransactionSchema,
});

const consensusDomainSchema = z.object({
	chain: z.bigint().nonnegative(),
	consensus: checkedAddressSchema,
});

export const safeTransactionPacketSchema = z.object({
	type: z.literal("safe_transaction_packet"),
	domain: consensusDomainSchema,
	proposal: transactionProposalSchema,
});

export type MetaTransaction = z.infer<typeof safeTransactionSchema>;

export type SafeTransactionPacket = z.infer<typeof safeTransactionPacketSchema>;
