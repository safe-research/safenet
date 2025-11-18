import z from "zod";
import { checkedAddressSchema } from "../../../types/schemas.js";

const rolloverSchema = z.object({
	activeEpoch: z.bigint().nonnegative(),
	proposedEpoch: z.bigint().nonnegative(),
	rolloverAt: z.bigint().nonnegative(),
	groupKeyX: z.bigint().nonnegative(),
	groupKeyY: z.bigint().nonnegative(),
});

const rolloverDomainSchema = z.object({
	chain: z.bigint().nonnegative(),
	consensus: checkedAddressSchema,
});

export const epochRolloverPacketSchema = z.object({
	type: z.literal("epoch_rollover_packet"),
	domain: rolloverDomainSchema,
	rollover: rolloverSchema,
});

export type EpochRolloverPacket = z.infer<typeof epochRolloverPacketSchema>;
