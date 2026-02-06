import { zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import type { SafeTransaction } from "../../schemas.js";
import { buildSignMessageChecks } from "./messages.js";

describe("buildSignMessageChecks", () => {
	it("should have at least one allowed address", async () => {
		expect(Object.keys(buildSignMessageChecks()).length).toBeGreaterThan(0);
	});

	it("should not allow calls", async () => {
		const tx: SafeTransaction = {
			chainId: 1n,
			safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			to: "0x6439e7ABD8Bb915A5263094784C5CF561c4172AC",
			value: 0n,
			data: "0x",
			operation: 0,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		};
		for (const check of Object.values(buildSignMessageChecks())) {
			expect(() => check(tx)).toThrow("Expected operation 1 got 0");
		}
	});

	it("should not allow unknown function call", async () => {
		const tx: SafeTransaction = {
			chainId: 1n,
			safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			to: "0x6439e7ABD8Bb915A5263094784C5CF561c4172AC",
			value: 0n,
			data: "0x5afe5afe",
			operation: 1,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		};
		for (const check of Object.values(buildSignMessageChecks())) {
			expect(() => check(tx)).toThrow("0x5afe5afe not supported");
		}
	});

	it("should allow signMessage function call", async () => {
		const tx: SafeTransaction = {
			chainId: 1n,
			safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			to: "0x6439e7ABD8Bb915A5263094784C5CF561c4172AC",
			value: 0n,
			data: "0x85a5affe",
			operation: 1,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		};
		for (const check of Object.values(buildSignMessageChecks())) {
			check(tx);
		}
	});
});
