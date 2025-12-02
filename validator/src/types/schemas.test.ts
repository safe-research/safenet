import { generatePrivateKey } from "viem/accounts";
import { describe, expect, it } from "vitest";
import { frostPointSchema } from "../consensus/schemas.js";
import { checkedAddressSchema, validatorConfigSchema } from "./schemas.js";

// --- Test Data ---

// This is a standard test address (e.g., from Hardhat/Anvil)
const MOCK_LOWERCASE_ADDRESS = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
const MOCK_WRONG_CHECKSUMMED_ADDRESS = "0xf39Fd6e51aad88F6F4ce6aB8827279cfFFb92266";
// This is the EIP-55 checksummed version of the address above
const MOCK_CHECKSUMMED_ADDRESS = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

const MOCK_INVALID_ADDRESS = "0xnotanaddress";
const MOCK_VALID_URL = "http://127.0.0.1:8545";
const MOCK_INVALID_URL = "not_a_real_url";

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
	it("should successfully parse a valid config object", () => {
		const pk = generatePrivateKey();
		const validConfig = {
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266,0x6Adb3baB5730852eB53987EA89D8e8f16393C200",
		};

		const result = validatorConfigSchema.parse(validConfig);
		expect(result).toEqual({
			RPC_URL: MOCK_VALID_URL,
			CONSENSUS_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			COORDINATOR_ADDRESS: MOCK_CHECKSUMMED_ADDRESS,
			CHAIN_ID: 100,
			PRIVATE_KEY: pk,
			PARTICIPANTS: ["0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", "0x6Adb3baB5730852eB53987EA89D8e8f16393C200"],
		});
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
			PARTICIPANTS: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266,0x6Adb3baB5730852eB53987EA89D8e8f16393C200",
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
});

describe("frostPointSchema", () => {
	it("should allow 0 points", () => {
		const validPoint = {
			x: 0n,
			y: 0n,
		};

		const result = frostPointSchema.safeParse(validPoint);

		expect(result.success).toBeTruthy();
	});

	it("should allow positive values", () => {
		const validPoint = {
			x: 1n,
			y: 1n,
		};

		const result = frostPointSchema.safeParse(validPoint);

		expect(result.success).toBeTruthy();
	});

	it("should not allow negative values for x", () => {
		const validPoint = {
			x: -1n,
			y: 0n,
		};

		const result = frostPointSchema.safeParse(validPoint);

		expect(result.success).toBeFalsy();
	});

	it("should not allow absence of x", () => {
		const validPoint = {
			y: 0n,
		};

		const result = frostPointSchema.safeParse(validPoint);

		expect(result.success).toBeFalsy();
	});

	it("should not allow negative values for y", () => {
		const validPoint = {
			x: 0n,
			y: -1n,
		};

		const result = frostPointSchema.safeParse(validPoint);

		expect(result.success).toBeFalsy();
	});

	it("should not allow absence of y", () => {
		const validPoint = {
			x: 0n,
		};

		const result = frostPointSchema.safeParse(validPoint);

		expect(result.success).toBeFalsy();
	});
});
