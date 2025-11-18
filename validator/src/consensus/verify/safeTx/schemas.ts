import z from "zod";
import { checkedAddressSchema, hexDataSchema } from "../../../types/schemas.js";

const safeMetaTransactionSchema = z.object({
	to: checkedAddressSchema,
	value: z.bigint().nonnegative(),
	data: hexDataSchema,
	operation: z.union([z.literal(0), z.literal(1)]),
});

const partialSafeTransactionSchema = z.object({
	...safeMetaTransactionSchema.shape,
	nonce: z.bigint().nonnegative(),
});

const safeDomainSchema = z.object({
	chain: z.bigint().nonnegative(),
	safe: checkedAddressSchema,
});

export const safeTransactionPacketSchema = z.object({
	type: z.literal("safe_transaction_packet"),
	domain: safeDomainSchema,
	transaction: partialSafeTransactionSchema,
});

export type SafeTransactionPacket = z.infer<typeof safeTransactionPacketSchema>;
