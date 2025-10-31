import { getAddress, type Hex, isHex } from "viem";
import z from "zod";

export const checkedAddressSchema = z
	.string()
	.transform((arg) => getAddress(arg));

export const hexDataSchema = z
	.string()
	.refine(isHex, "Value is not a valid hex string")
	.transform((val) => val as Hex);

export const validatorConfigSchema = z.object({
	RPC_URL: z.url(),
	CONSENSUS_CORE_ADDRESS: checkedAddressSchema,
});
