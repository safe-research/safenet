import { ethAddress, zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import type { SafeTransaction } from "../../schemas.js";
import { buildSetFallbackHandlerCheck } from "./fallback.js";

describe("fallback", () => {
	describe("buildSetFallbackHandlerCheck", () => {
		it("should throw if unsupported data", async () => {
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: "0x5afe",
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSetFallbackHandlerCheck();
			expect(Object.keys(check)).toStrictEqual(["0xf08a0323"]);
			expect(() => check["0xf08a0323"](tx)).toThrow();
		});

		it("should throw for unknown fallback handler", async () => {
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: `0xf08a0323000000000000000000000000${ethAddress.slice(2)}`,
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSetFallbackHandlerCheck();
			expect(Object.keys(check)).toStrictEqual(["0xf08a0323"]);
			expect(() => check["0xf08a0323"](tx)).toThrow(
				"Cannot set unknown fallback handler 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE",
			);
		});

		it("allow zero address as fallback handler (to reset)", async () => {
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: "0xf08a03230000000000000000000000000000000000000000000000000000000000000000",
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSetFallbackHandlerCheck();
			expect(Object.keys(check)).toStrictEqual(["0xf08a0323"]);
			check["0xf08a0323"](tx);
		});
	});
});
