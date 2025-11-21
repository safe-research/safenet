import z from "zod";
import { checkedAddressSchema } from "../../../types/schemas.js";

const rolloverSchema = z.object({
	activeEpoch: z.bigint().nonnegative(),
	proposedEpoch: z.bigint().nonnegative(),
	rolloverBlock: z.bigint().nonnegative(),
	groupKeyX: z.bigint().nonnegative(),
	groupKeyY: z.bigint().nonnegative(),
});

const consensusDomainSchema = z.object({
	chain: z.bigint().nonnegative(),
	consensus: checkedAddressSchema,
});

export const epochRolloverPacketSchema = z.object({
	type: z.literal("epoch_rollover_packet"),
	domain: consensusDomainSchema,
	rollover: rolloverSchema,
});

export type EpochRolloverPacket = z.infer<typeof epochRolloverPacketSchema>;
