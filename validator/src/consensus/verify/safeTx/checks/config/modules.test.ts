import { ethAddress, zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import type { SafeTransaction } from "../../schemas.js";
import { buildEnableModuleCheck, buildSetModuleGuardCheck } from "./modules.js";

describe("modules", () => {
	describe("buildEnableModuleCheck", () => {
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
			const check = buildEnableModuleCheck();
			expect(Object.keys(check)).toStrictEqual(["0x610b5925"]);
			expect(() => check["0x610b5925"](tx)).toThrow();
		});

		it("should throw for zero address as a module", async () => {
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: "0x610b59250000000000000000000000000000000000000000000000000000000000000000",
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildEnableModuleCheck();
			expect(Object.keys(check)).toStrictEqual(["0x610b5925"]);
			expect(() => check["0x610b5925"](tx)).toThrow(
				"Cannot enable unknown module 0x0000000000000000000000000000000000000000",
			);
		});

		it("should throw for unknown module", async () => {
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: `0x610b5925000000000000000000000000${ethAddress.slice(2)}`,
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildEnableModuleCheck();
			expect(Object.keys(check)).toStrictEqual(["0x610b5925"]);
			expect(() => check["0x610b5925"](tx)).toThrow(
				"Cannot enable unknown module 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE",
			);
		});
	});

	describe("buildSetModuleGuardCheck", () => {
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
			const check = buildSetModuleGuardCheck();
			expect(Object.keys(check)).toStrictEqual(["0xe068df37"]);
			expect(() => check["0xe068df37"](tx)).toThrow();
		});

		it("should throw for unknown guard", async () => {
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: `0xe068df37000000000000000000000000${ethAddress.slice(2)}`,
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSetModuleGuardCheck();
			expect(Object.keys(check)).toStrictEqual(["0xe068df37"]);
			expect(() => check["0xe068df37"](tx)).toThrow(
				"Cannot set unknown module guard 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE",
			);
		});

		it("allow zero address as module guard (to reset)", async () => {
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: "0xe068df370000000000000000000000000000000000000000000000000000000000000000",
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSetModuleGuardCheck();
			expect(Object.keys(check)).toStrictEqual(["0xe068df37"]);
			check["0xe068df37"](tx);
		});
	});
});
