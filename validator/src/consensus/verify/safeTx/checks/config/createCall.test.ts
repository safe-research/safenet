import { zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import type { SafeTransaction } from "../../schemas.js";
import { buildCreateCallChecks } from "./createCall.js";

describe("buildCreateCallChecks", () => {
	it("should have at least one allowed address", async () => {
		expect(Object.keys(buildCreateCallChecks()).length).toBeGreaterThan(0);
	});

	it("should not allow calls", async () => {
		const tx: SafeTransaction = {
			chainId: 1n,
			safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			to: "0x2Ef5ECfbea521449E4De05EDB1ce63B75eDA90B4",
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
		for (const check of Object.values(buildCreateCallChecks())) {
			expect(() => check(tx)).toThrow("Expected operation 1 got 0");
		}
	});

	it("should not allow unknown function call", async () => {
		const tx: SafeTransaction = {
			chainId: 1n,
			safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			to: "0x2Ef5ECfbea521449E4De05EDB1ce63B75eDA90B4",
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
		for (const check of Object.values(buildCreateCallChecks())) {
			expect(() => check(tx)).toThrow("0x5afe5afe not supported");
		}
	});

	it("should allow performCreate delegatecall", async () => {
		const tx: SafeTransaction = {
			chainId: 1n,
			safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			to: "0x2Ef5ECfbea521449E4De05EDB1ce63B75eDA90B4",
			value: 0n,
			data: "0x4c8c9ea1", // performCreate(uint256,bytes)
			operation: 1,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		};
		for (const check of Object.values(buildCreateCallChecks())) {
			check(tx);
		}
	});

	it("should allow performCreate2 delegatecall", async () => {
		const tx: SafeTransaction = {
			chainId: 1n,
			safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			to: "0x2Ef5ECfbea521449E4De05EDB1ce63B75eDA90B4",
			value: 0n,
			data: "0x4847be6f", // performCreate2(uint256,bytes,bytes32)
			operation: 1,
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce: 0n,
		};
		for (const check of Object.values(buildCreateCallChecks())) {
			check(tx);
		}
	});
});
