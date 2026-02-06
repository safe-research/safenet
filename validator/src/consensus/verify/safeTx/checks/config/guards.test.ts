import { ethAddress, zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import type { SafeTransaction } from "../../schemas.js";
import { buildSetGuardCheck } from "./guards.js";

describe("guards", () => {
	describe("buildSetGuardCheck", () => {
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
			const check = buildSetGuardCheck();
			expect(Object.keys(check)).toStrictEqual(["0xe19a9dd9"]);
			expect(() => check["0xe19a9dd9"](tx)).toThrow();
		});

		it("should throw for unknown guard", async () => {
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: `0xe19a9dd9000000000000000000000000${ethAddress.slice(2)}`,
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSetGuardCheck();
			expect(Object.keys(check)).toStrictEqual(["0xe19a9dd9"]);
			expect(() => check["0xe19a9dd9"](tx)).toThrow(
				"Cannot set unknown guard 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE",
			);
		});

		it("allow zero address as guard (to reset)", async () => {
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: "0xe19a9dd90000000000000000000000000000000000000000000000000000000000000000",
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSetGuardCheck();
			expect(Object.keys(check)).toStrictEqual(["0xe19a9dd9"]);
			check["0xe19a9dd9"](tx);
		});
	});
});
