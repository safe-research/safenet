import { type Address, getAddress, isAddress } from "viem";
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

export const validatorConfigSchema = z.object({
	RPC_URL: z.url(),
	CONSENSUS_CORE_ADDRESS: checkedAddressSchema,
});
