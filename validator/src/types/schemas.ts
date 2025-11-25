import { type Address, getAddress, type Hex, isAddress, isHex } from "viem";
import { z } from "zod";
import { supportedChains } from "./chains.js";

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

export const hexDataSchema = z
	.string()
	.refine(isHex, "Value is not a valid hex string")
	.transform((val) => val as Hex);

export const supportedChainsSchema = z.coerce
	.number()
	.pipe(z.union(supportedChains.map((chain) => z.literal(chain.id))));

export const participantsSchema = z.preprocess((val) => {
	if (typeof val === "string") {
		return val.split(",");
	}
	return val;
}, z.array(checkedAddressSchema));

export const validatorConfigSchema = z.object({
	RPC_URL: z.url(),
	PRIVATE_KEY: hexDataSchema,
	CONSENSUS_ADDRESS: checkedAddressSchema,
	COORDINATOR_ADDRESS: checkedAddressSchema,
	CHAIN_ID: supportedChainsSchema,
	PARTICIPANTS: participantsSchema,
});

export type SupportedChain = z.infer<typeof supportedChainsSchema>;
