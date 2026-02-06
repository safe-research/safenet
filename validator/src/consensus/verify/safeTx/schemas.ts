import z from "zod";
import { checkedAddressSchema, hexDataSchema } from "../../../types/schemas.js";

export const safeTransactionSchema = z.object({
	chainId: z.bigint().nonnegative(),
	safe: checkedAddressSchema,
	to: checkedAddressSchema,
	value: z.bigint().nonnegative(),
	data: hexDataSchema,
	operation: z.union([z.literal(0), z.literal(1)]),
	safeTxGas: z.bigint().nonnegative(),
	baseGas: z.bigint().nonnegative(),
	gasPrice: z.bigint().nonnegative(),
	gasToken: checkedAddressSchema,
	refundReceiver: checkedAddressSchema,
	nonce: z.bigint().nonnegative(),
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

export type SafeTransaction = z.infer<typeof safeTransactionSchema>;

export type SafeTransactionPacket = z.infer<typeof safeTransactionPacketSchema>;
