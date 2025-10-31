import { getAddress } from "viem";
import { z } from "zod";

export const checkedAddressSchema = z
	.string()
	.transform((arg) => getAddress(arg));

export const validatorConfigSchema = z.object({
	RPC_URL: z.url(),
	CONSENSUS_CORE_ADDRESS: checkedAddressSchema,
});
