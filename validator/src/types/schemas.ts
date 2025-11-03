import { type Address, checksumAddress, isAddress } from "viem";
import { z } from "zod";

export const checkedAddressSchema = z
	.string()
	.refine((arg) => isAddress(arg))
	.transform((arg) => checksumAddress(arg as Address));

export const validatorConfigSchema = z.object({
	RPC_URL: z.url(),
	CONSENSUS_CORE_ADDRESS: checkedAddressSchema,
});
