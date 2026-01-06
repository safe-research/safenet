import { describe, expect, it } from "vitest";
import type { MetaTransaction } from "../../schemas.js";
import { buildSingletonUpgradeChecks } from "./singletons.js";

describe("buildSingletonUpgradeChecks", () => {
	it("should have at least one allowed address", async () => {
		expect(Object.keys(buildSingletonUpgradeChecks()).length).toBeGreaterThan(0);
	});

	it("should not allow calls", async () => {
		const tx: MetaTransaction = {
			to: "0x6439e7ABD8Bb915A5263094784C5CF561c4172AC",
			value: 0n,
			data: "0x",
			operation: 0,
			nonce: 0n,
			chainId: 1n,
			account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
		};
		for (const check of Object.values(buildSingletonUpgradeChecks())) {
			expect(() => check(tx)).toThrow("Expected operation 1 got 0");
		}
	});

	it("should not allow unknown function call", async () => {
		const tx: MetaTransaction = {
			to: "0x6439e7ABD8Bb915A5263094784C5CF561c4172AC",
			value: 0n,
			data: "0x5afe5afe",
			operation: 1,
			nonce: 0n,
			chainId: 1n,
			account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
		};
		for (const check of Object.values(buildSingletonUpgradeChecks())) {
			expect(() => check(tx)).toThrow("0x5afe5afe not supported");
		}
	});

	it("should allow migrateSingleton function call", async () => {
		const tx: MetaTransaction = {
			to: "0x6439e7ABD8Bb915A5263094784C5CF561c4172AC",
			value: 0n,
			data: "0xf6682ab0",
			operation: 1,
			nonce: 0n,
			chainId: 1n,
			account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
		};
		for (const check of Object.values(buildSingletonUpgradeChecks())) {
			check(tx);
		}
	});

	it("should allow migrateWithFallbackHandler function call", async () => {
		const tx: MetaTransaction = {
			to: "0x6439e7ABD8Bb915A5263094784C5CF561c4172AC",
			value: 0n,
			data: "0xed007fc6",
			operation: 1,
			nonce: 0n,
			chainId: 1n,
			account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
		};
		for (const check of Object.values(buildSingletonUpgradeChecks())) {
			check(tx);
		}
	});

	it("should allow migrateL2Singleton function call", async () => {
		const tx: MetaTransaction = {
			to: "0x6439e7ABD8Bb915A5263094784C5CF561c4172AC",
			value: 0n,
			data: "0x07f464a4",
			operation: 1,
			nonce: 0n,
			chainId: 1n,
			account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
		};
		for (const check of Object.values(buildSingletonUpgradeChecks())) {
			check(tx);
		}
	});

	it("should allow migrateL2WithFallbackHandler function call", async () => {
		const tx: MetaTransaction = {
			to: "0x6439e7ABD8Bb915A5263094784C5CF561c4172AC",
			value: 0n,
			data: "0x68cb3d94",
			operation: 1,
			nonce: 0n,
			chainId: 1n,
			account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
		};
		for (const check of Object.values(buildSingletonUpgradeChecks())) {
			check(tx);
		}
	});
});
