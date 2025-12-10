import { type Address, getAddress, type Hex, isAddress, isHex, size, zeroHash } from "viem";
import { z } from "zod";
import { supportedChains } from "./chains.js";

export const checkedAddressSchema = z
	.string()
	// Strict is disabled here, as a manual check for the correct checksum is performed.
	// Viem always allows to get around the checksum when providing an all lowercase address.
	// With strict to `false` and the manual check the additional overhead is minimal as the
	// result of `isAddress` is cached internaly in Viem (strict is part of the cache key).
	.refine((arg) => isAddress(arg, { strict: false }) && arg === getAddress(arg), "Invalid address format or checksum")
	.transform((arg) => arg as Address);

export const hexDataSchema = z
	.string()
	.refine(isHex, "Value is not a valid hex string")
	.transform((val) => val as Hex);

export const hexBytes32Schema = hexDataSchema.refine((bytes) => size(bytes) === 32, "Value is not 32 bytes long");

export const supportedChainsSchema = z.coerce
	.number()
	.pipe(z.union(supportedChains.map((chain) => z.literal(chain.id))));

export const participantsSchema = z
	.preprocess((val) => {
		if (typeof val === "string") {
			return val.split(",");
		}
		return val;
	}, z.array(checkedAddressSchema))
	.transform((participants) => participants.map((address, i) => ({ address, id: BigInt(i + 1) })));

export const genesisSaltSchema = z.preprocess((val) => {
	if (val === undefined || val === "") {
		return zeroHash;
	}
	return val;
}, hexBytes32Schema);

export const validatorConfigSchema = z.object({
	RPC_URL: z.url(),
	PRIVATE_KEY: hexBytes32Schema,
	CONSENSUS_ADDRESS: checkedAddressSchema,
	COORDINATOR_ADDRESS: checkedAddressSchema,
	CHAIN_ID: supportedChainsSchema,
	PARTICIPANTS: participantsSchema,
	GENESIS_SALT: genesisSaltSchema,
});

export const chunked = <T>(sz: number, transform: (b: Buffer) => T): ((b: Buffer) => T[]) => {
	return (b: Buffer) => {
		if (b.length % sz !== 0) {
			throw new Error(`buffer of length ${b.length} cannot be chunked in ${sz} bytes`);
		}
		return [...Array(b.length / sz)].map((_, i) => {
			const start = i * sz;
			const end = start + sz;
			return transform(b.subarray(start, end));
		});
	};
};

export type SupportedChain = z.infer<typeof supportedChainsSchema>;
