import { type Address, getAddress, type Hex, isAddress, isHex, size, zeroHash } from "viem";
import { type ZodType, z } from "zod";
import { supportedChains } from "./chains.js";

export const logLevelSchema = z.enum(["error", "warn", "notice", "info", "debug", "silly", "silent"]);
export const portSchema = z.preprocess((val) => {
	if (val === "") {
		return undefined;
	}
	if (typeof val === "string") {
		return Number.parseInt(val);
	}
	return val;
}, z.int().gte(0).lte(65535).optional());

export const fileNameSchema = z.preprocess((val) => (val === "" ? undefined : val), z.string().optional());

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

const supportedChainsSchema = z.coerce.number().pipe(z.union(supportedChains.map((chain) => z.literal(chain.id))));

const participantsSchema = z
	.preprocess((val) => {
		if (typeof val === "string") {
			return val.split(",");
		}
		return val;
	}, z.array(checkedAddressSchema))
	.transform((participants) => participants.map((address, i) => ({ address, id: BigInt(i + 1) })));

const genesisSaltSchema = z.preprocess((val) => {
	if (val === undefined || val === "") {
		return zeroHash;
	}
	return val;
}, hexBytes32Schema);

const BLOCKTIME_IN_SECONDS = 5n; // value assumed for gnosis chain
const BLOCKS_PER_EPOCH = (24n * 60n * 60n) / BLOCKTIME_IN_SECONDS; // ~ blocks for 1 day

const emptyToDefault = <T>(schema: ZodType<T>, defaultVal?: unknown): ZodType<T> =>
	z.preprocess((val) => {
		if (val === undefined || val === "") {
			return defaultVal;
		}
		return val;
	}, schema);

export const validatorConfigSchema = z.object({
	LOG_LEVEL: logLevelSchema.optional(),
	METRICS_PORT: portSchema,
	STORAGE_FILE: fileNameSchema,
	RPC_URL: z.url(),
	PRIVATE_KEY: hexBytes32Schema,
	CONSENSUS_ADDRESS: checkedAddressSchema,
	COORDINATOR_ADDRESS: checkedAddressSchema,
	CHAIN_ID: supportedChainsSchema,
	PARTICIPANTS: participantsSchema,
	GENESIS_SALT: genesisSaltSchema,
	BLOCKS_PER_EPOCH: emptyToDefault(z.coerce.bigint(), BLOCKS_PER_EPOCH),
	BASE_FEE_MULTIPLIER: emptyToDefault(z.coerce.number().optional()),
	PRIORITY_FEE_PER_GAS: emptyToDefault(z.coerce.bigint().optional()),
	MAX_REORG_DEPTH: emptyToDefault(z.coerce.number().int().optional()),
	BLOCK_PAGE_SIZE: emptyToDefault(z.coerce.number().int().optional()),
	MAX_LOGS_PER_QUERY: emptyToDefault(z.coerce.number().int().optional()),
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
