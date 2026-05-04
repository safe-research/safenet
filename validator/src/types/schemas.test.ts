import { parseGwei } from "viem";
import { generatePrivateKey } from "viem/accounts";
import { describe, expect, it } from "vitest";
import { frostPointSchema } from "../machine/transitions/schemas.js";
import { checkedAddressSchema, validatorConfigSchema } from "./schemas.js";

// --- Test Data ---

// This is a standard test address (e.g., from Hardhat/Anvil)
const MOCK_LOWERCASE_ADDRESS = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
const MOCK_WRONG_CHECKSUMMED_ADDRESS = "0xf39Fd6e51aad88F6F4ce6aB8827279cfFFb92266";
// This is the EIP-55 checksummed version of the address above
const MOCK_CHECKSUMMED_ADDRESS = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const MOCK_GENESIS_SALT = "0x5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe5afe";

const MOCK_INVALID_ADDRESS = "0xnotanaddress";
const MOCK_VALID_URL = "http://127.0.0.1:8545";
const MOCK_INVALID_URL = "not_a_real_url";
const VALID_PARTICIPANTS_INPUT = `[{ "address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266" }, { "address": "0x6Adb3baB5730852eB53987EA89D8e8f16393C200" }]`;
const VALID_PARTICIPANTS_OUTPUT = [
	{ address: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", activeFrom: 0n },
	{ address: "0x6Adb3baB5730852eB53987EA89D8e8f16393C200", activeFrom: 0n },
];

// --- Tests ---
describe("checkedAddressSchema", () => {
	it("should successfully parse a valid, already-checksummed address", () => {
		const parsedAddress = checkedAddressSchema.parse(MOCK_CHECKSUMMED_ADDRESS);

		// Expect the output to remain checksummed
		expect(parsedAddress).toBe(MOCK_CHECKSUMMED_ADDRESS);
	});

	it("should fail to parse an invalid address string", () => {
		// .safeParse() is better for testing failures as it doesn't throw
		const result = checkedAddressSchema.safeParse(MOCK_INVALID_ADDRESS);

		expect(result.success).toBe(false);
	});

	it("should fail to parse non-checksummed address", () => {
		const result = checkedAddressSchema.safeParse(MOCK_LOWERCASE_ADDRESS);

		expect(result.success).toBe(false);
	});

	it("should fail to parse a wrongly checksummed address", () => {
		const result = checkedAddressSchema.safeParse(MOCK_WRONG_CHECKSUMMED_ADDRESS);

		expect(result.success).toBe(false);
	});

	it("should fail to parse an address that is too short", () => {
		const result = checkedAddressSchema.safeParse("0x12345");

		expect(result.success).toBe(false);
	});

	it("should fail to parse a non-string input", () => {
		const result = checkedAddressSchema.safeParse(123456789);

		expect(result.success).toBe(false);
		if (!result.success) {
			expect(result.error.issues).toHaveLength(1);
			const issue = result.error.issues[0];
			expect(issue.code).toBe("invalid_type");
			expect(issue.message).toBe("Invalid input: expected string, received number");
		}
	});
});

describe("validatorConfigSchema", () => {
	it("should successfully parse a valid config object without blocks per epoch", () => {
		const pk = generatePrivateKey();
		const validConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			STAKER_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
		};

		const result = validatorConfigSchema.parse(validConfig);
		expect(result).toEqual({
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			STAKER_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			PARTICIPANTS: VALID_PARTICIPANTS_OUTPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: 17280n,
			ALLOWED_ORACLES: [],
			COMMIT_SHA: "unknown",
		});
	});

	it("should successfully parse a valid config object with empty blocks per epoch", () => {
		const pk = generatePrivateKey();
		const validConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: "",
			BLOCKS_BEFORE_RESUBMIT: "",
			SKIP_GENESIS: "",
		};

		const result = validatorConfigSchema.parse(validConfig);
		expect(result).toEqual({
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_OUTPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: 17280n,
			ALLOWED_ORACLES: [],
			COMMIT_SHA: "unknown",
		});
	});

	it("should successfully parse a valid config object with blocks per epoch", () => {
		const pk = generatePrivateKey();
		const validConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: "100",
			BLOCKS_BEFORE_RESUBMIT: "200",
		};

		const result = validatorConfigSchema.parse(validConfig);
		expect(result).toEqual({
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_OUTPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: 100n,
			BLOCKS_BEFORE_RESUBMIT: 200n,
			ALLOWED_ORACLES: [],
			COMMIT_SHA: "unknown",
		});
	});

	it("should successfully parse a valid config object with empty timeouts", () => {
		const pk = generatePrivateKey();
		const validConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			KEY_GEN_TIMEOUT: "",
			SIGNING_TIMEOUT: "",
		};

		const result = validatorConfigSchema.parse(validConfig);
		expect(result).toEqual({
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_OUTPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: 17280n,
			ALLOWED_ORACLES: [],
			COMMIT_SHA: "unknown",
		});
	});

	it("should successfully parse a valid config object with valid timeout parameters", () => {
		const pk = generatePrivateKey();
		const validConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			KEY_GEN_TIMEOUT: "1000",
			SIGNING_TIMEOUT: "253",
		};

		const result = validatorConfigSchema.parse(validConfig);
		expect(result).toEqual({
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_OUTPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: 17280n,
			KEY_GEN_TIMEOUT: 1000n,
			SIGNING_TIMEOUT: 253n,
			ALLOWED_ORACLES: [],
			COMMIT_SHA: "unknown",
		});
	});

	it("should successfully parse a valid config object with empty fee parameters", () => {
		const pk = generatePrivateKey();
		const validConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BASE_FEE_MULTIPLIER: "",
			PRIORITY_FEE_PER_GAS: "",
		};

		const result = validatorConfigSchema.parse(validConfig);
		expect(result).toEqual({
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_OUTPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: 17280n,
			ALLOWED_ORACLES: [],
			COMMIT_SHA: "unknown",
		});
	});

	it("should successfully parse a valid config object with valid fee parameters", () => {
		const pk = generatePrivateKey();
		const validConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BASE_FEE_MULTIPLIER: "2.4",
			PRIORITY_FEE_PER_GAS: parseGwei("0.1"),
		};

		const result = validatorConfigSchema.parse(validConfig);
		expect(result).toEqual({
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_OUTPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: 17280n,
			BASE_FEE_MULTIPLIER: 2.4,
			PRIORITY_FEE_PER_GAS: 100000000n,
			ALLOWED_ORACLES: [],
			COMMIT_SHA: "unknown",
		});
	});

	it("should fail if fee params are invalid", () => {
		const invalidConfig = {
			BASE_FEE_MULTIPLIER: "2.4foo",
			PRIORITY_FEE_PER_GAS: "0.1",
		};

		const result = validatorConfigSchema.safeParse(invalidConfig);
		expect(result.success).toBe(false);

		// Check that the error is specifically about the RPC_URL
		if (!result.success) {
			const multiplierError = result.error.issues.find((issue) => issue.path[0] === "BASE_FEE_MULTIPLIER");
			expect(multiplierError).toBeDefined();
			expect(multiplierError?.message).toBe("Invalid input: expected number, received NaN");

			const priorityFeeError = result.error.issues.find((issue) => issue.path[0] === "PRIORITY_FEE_PER_GAS");
			expect(priorityFeeError).toBeDefined();
			expect(priorityFeeError?.message).toBe("Invalid input: expected bigint, received string");
		}
	});

	it("should correctly parse skip genesis parameter", () => {
		const pk = generatePrivateKey();
		const baseConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BASE_FEE_MULTIPLIER: "",
			PRIORITY_FEE_PER_GAS: "",
		};

		const expectedBaseConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_OUTPUT,
			GENESIS_SALT: MOCK_GENESIS_SALT,
			BLOCKS_PER_EPOCH: 17280n,
			ALLOWED_ORACLES: [],
			COMMIT_SHA: "unknown",
		};

		expect(
			validatorConfigSchema.parse({
				...baseConfig,
				SKIP_GENESIS: "false",
			}),
		).toEqual({
			...expectedBaseConfig,
			SKIP_GENESIS: false,
		});

		expect(
			validatorConfigSchema.parse({
				...baseConfig,
				SKIP_GENESIS: "0",
			}),
		).toEqual({
			...expectedBaseConfig,
			SKIP_GENESIS: false,
		});

		expect(
			validatorConfigSchema.parse({
				...baseConfig,
				SKIP_GENESIS: "true",
			}),
		).toEqual({
			...expectedBaseConfig,
			SKIP_GENESIS: true,
		});

		expect(
			validatorConfigSchema.parse({
				...baseConfig,
				SKIP_GENESIS: "1",
			}),
		).toEqual({
			...expectedBaseConfig,
			SKIP_GENESIS: true,
		});
	});

	it("should fail if SKIP_GENESIS is invalid", () => {
		const invalidConfig = {
			SKIP_GENESIS: 1,
		};

		const result = validatorConfigSchema.safeParse(invalidConfig);
		expect(result.success).toBe(false);

		// Check that the error is specifically about the SKIP_GENESIS
		if (!result.success) {
			const urlError = result.error.issues.find((issue) => issue.path[0] === "SKIP_GENESIS");
			expect(urlError).toBeDefined();
			expect(urlError?.message).toBe('Invalid option: expected one of "0"|"1"|"true"|"false"');
		}
	});

	it("should fail if RPC_URL is invalid", () => {
		const invalidConfig = {
			RPC_URL: MOCK_INVALID_URL, // <-- Invalid
			CONSENSUS_ADDRESS: MOCK_LOWERCASE_ADDRESS,
		};

		const result = validatorConfigSchema.safeParse(invalidConfig);
		expect(result.success).toBe(false);

		// Check that the error is specifically about the RPC_URL
		if (!result.success) {
			const urlError = result.error.issues.find((issue) => issue.path[0] === "RPC_URL");
			expect(urlError).toBeDefined();
			expect(urlError?.message).toBe("Invalid URL");
		}
	});

	it("should fail if CHAIN_ID is not from a supported network", () => {
		const pk = generatePrivateKey();
		const invalidConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 1, // Mainnet is not supported right now
			PRIVATE_KEY: pk,
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
		};

		const result = validatorConfigSchema.safeParse(invalidConfig);
		expect(result.success).toBe(false);

		// Check that the error is from the address field
		if (!result.success) {
			const addressError = result.error.issues.find((issue) => issue.path[0] === "CHAIN_ID");
			expect(addressError).toBeDefined();
		}
	});

	it("should fail if CONSENSUS_ADDRESS is invalid", () => {
		const invalidConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_INVALID_ADDRESS, // <-- Invalid
		};

		const result = validatorConfigSchema.safeParse(invalidConfig);
		expect(result.success).toBe(false);

		// Check that the error is from the address field
		if (!result.success) {
			const addressError = result.error.issues.find((issue) => issue.path[0] === "CONSENSUS_ADDRESS");
			expect(addressError).toBeDefined();
		}
	});

	it("should fail if RPC_URL is missing", () => {
		const incompleteConfig = {
			CONSENSUS_ADDRESS: MOCK_LOWERCASE_ADDRESS,
		};

		const result = validatorConfigSchema.safeParse(incompleteConfig);
		expect(result.success).toBe(false);

		if (!result.success) {
			const error = result.error.issues.find((issue) => issue.path[0] === "RPC_URL");
			expect(error).toBeDefined();
			expect(error?.code).toBe("invalid_type");
			expect(error?.message).toBe("Invalid input: expected string, received undefined");
		}
	});

	it("should fail if CONSENSUS_ADDRESS is missing", () => {
		const incompleteConfig = {
			RPC_URL: MOCK_VALID_URL,
		};

		const result = validatorConfigSchema.safeParse(incompleteConfig);
		expect(result.success).toBe(false);

		if (!result.success) {
			const error = result.error.issues.find((issue) => issue.path[0] === "CONSENSUS_ADDRESS");
			expect(error).toBeDefined();
			expect(error?.code).toBe("invalid_type");
			expect(error?.message).toBe("Invalid input: expected string, received undefined");
		}
	});

	it("should fail if STAKER_ADDRESS is invalid", () => {
		const invalidConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: generatePrivateKey(),
			STAKER_ADDRESS: MOCK_INVALID_ADDRESS, // <-- Invalid
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
		};

		const result = validatorConfigSchema.safeParse(invalidConfig);
		expect(result.success).toBe(false);

		if (!result.success) {
			const addressError = result.error.issues.find((issue) => issue.path[0] === "STAKER_ADDRESS");
			expect(addressError).toBeDefined();
		}
	});

	it("should use default values for optional fields", () => {
		const config = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: generatePrivateKey(),
			PARTICIPANTS: VALID_PARTICIPANTS_INPUT,
		};

		const optionals = [
			"METRICS_HOST",
			"METRICS_PORT",
			"STORAGE_FILE",
			"STAKER_ADDRESS",
			"BLOCKS_BEFORE_RESUBMIT",
			"BASE_FEE_MULTIPLIER",
			"PRIORITY_FEE_PER_GAS",
			"BLOCK_TIME_OVERRIDE",
			"MAX_REORG_DEPTH",
			"BLOCK_PAGE_SIZE",
			"BLOCK_ALL_LOGS_QUERY_RETRY_COUNT",
			"BLOCK_SINGLE_QUERY_RETRY_COUNT",
			"KEY_GEN_TIMEOUT",
			"SIGNING_TIMEOUT",
			"MAX_LOGS_PER_QUERY",
			"SKIP_GENESIS",
		] as const;

		for (const settings of [{}, Object.fromEntries(optionals.map((key) => [key, ""]))]) {
			const parsed = validatorConfigSchema.parse({ ...config, ...settings });
			for (const optional of optionals) {
				expect(parsed[optional]).toBeUndefined();
			}
		}
	});
});

describe("frostPointSchema", () => {
	it("should not allow 0 points", () => {
		const invalidPoint = {
			x: 0n,
			y: 0n,
		};

		const result = frostPointSchema.safeParse(invalidPoint);

		expect(result.success).toBeFalsy();
	});

	it("should allow positive values", () => {
		const validPoint = {
			x: 8157951670743782207572742157759285246997125817591478561509454646417563755134n,
			y: 56888799465634869784517292721691123160415451366201038719887189136540242661500n,
		};

		const result = frostPointSchema.safeParse(validPoint);

		expect(result.success).toBeTruthy();
	});

	it("should not allow negative values for x", () => {
		const invalidPoint = {
			x: -8157951670743782207572742157759285246997125817591478561509454646417563755134n,
			y: 56888799465634869784517292721691123160415451366201038719887189136540242661500n,
		};

		const result = frostPointSchema.safeParse(invalidPoint);

		expect(result.success).toBeFalsy();
	});

	it("should not allow absence of x", () => {
		const invalidPoint = {
			y: 0n,
		};

		const result = frostPointSchema.safeParse(invalidPoint);

		expect(result.success).toBeFalsy();
	});

	it("should not allow negative values for y", () => {
		const invalidPoint = {
			x: 8157951670743782207572742157759285246997125817591478561509454646417563755134n,
			y: -56888799465634869784517292721691123160415451366201038719887189136540242661500n,
		};

		const result = frostPointSchema.safeParse(invalidPoint);

		expect(result.success).toBeFalsy();
	});

	it("should not allow absence of y", () => {
		const invalidPoint = {
			x: 0n,
		};

		const result = frostPointSchema.safeParse(invalidPoint);

		expect(result.success).toBeFalsy();
	});
});
