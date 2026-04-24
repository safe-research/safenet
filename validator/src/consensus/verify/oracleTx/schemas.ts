import z from "zod";
import { checkedAddressSchema } from "../../../types/schemas.js";
import { safeTransactionSchema } from "../safeTx/schemas.js";

const consensusDomainSchema = z.object({
	chain: z.bigint().nonnegative(),
	consensus: checkedAddressSchema,
});

const oracleTransactionProposalSchema = z.object({
	epoch: z.bigint().nonnegative(),
	oracle: checkedAddressSchema,
	transaction: safeTransactionSchema,
});

export const oracleTransactionPacketSchema = z.object({
	type: z.literal("oracle_transaction_packet"),
	domain: consensusDomainSchema,
	proposal: oracleTransactionProposalSchema,
});

export type OracleTransactionPacket = z.infer<typeof oracleTransactionPacketSchema>;
