import { ethAddress, type Hex, zeroAddress } from "viem";
import { describe, expect, it, vi } from "vitest";
import type { TransactionCheck } from "../handler.js";
import type { SafeTransaction } from "../schemas.js";
import {
	buildFixedParamsCheck,
	buildNoDelegateCallCheck,
	buildSelectorChecks,
	buildSupportedSelectorCheck,
	buildSupportedSignaturesCheck,
} from "./basic.js";

describe("basic checks", () => {
	describe("buildNoDelegateCallCheck", () => {
		it("should throw for delegatecalls", async () => {
			const check = buildNoDelegateCallCheck();
			expect(() =>
				check({
					chainId: 1n,
					safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
					to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
					value: 0n,
					data: "0x5afe",
					operation: 1,
					safeTxGas: 0n,
					baseGas: 0n,
					gasPrice: 0n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
					nonce: 0n,
				}),
			).toThrowError(Error("Delegatecall not allowed"));
		});

		it("should forward continue for call transactions", async () => {
			const check = buildNoDelegateCallCheck();
			check({
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			});
		});
	});
	describe("buildFixedParamsCheck", () => {
		it("should throw for wrong operation", async () => {
			const check = buildFixedParamsCheck({
				operation: 0,
			});
			expect(() =>
				check({
					chainId: 1n,
					safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
					to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
					value: 0n,
					data: "0x5afe",
					operation: 1,
					safeTxGas: 0n,
					baseGas: 0n,
					gasPrice: 0n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
					nonce: 0n,
				}),
			).toThrowError(Error("Expected operation 0 got 1"));
		});

		it("should throw for wrong to", async () => {
			const check = buildFixedParamsCheck({
				to: ethAddress,
			});
			expect(() =>
				check({
					chainId: 1n,
					safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
					to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
					value: 0n,
					data: "0x5afe",
					operation: 0,
					safeTxGas: 0n,
					baseGas: 0n,
					gasPrice: 0n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
					nonce: 0n,
				}),
			).toThrowError(Error(`Expected to ${ethAddress} got 0x40A2aCCbd92BCA938b02010E17A5b8929b49130D`));
		});

		it("should throw for wrong data", async () => {
			const check = buildFixedParamsCheck({
				data: "0x5afe5afe",
			});
			expect(() =>
				check({
					chainId: 1n,
					safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
					to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
					value: 0n,
					data: "0x5afe",
					operation: 0,
					safeTxGas: 0n,
					baseGas: 0n,
					gasPrice: 0n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
					nonce: 0n,
				}),
			).toThrowError(Error("Expected data 0x5afe5afe got 0x5afe"));
		});

		it("should throw for wrong value", async () => {
			const check = buildFixedParamsCheck({
				value: 0n,
			});
			expect(() =>
				check({
					chainId: 1n,
					safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
					to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
					value: 1n,
					data: "0x5afe",
					operation: 0,
					safeTxGas: 0n,
					baseGas: 0n,
					gasPrice: 0n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
					nonce: 0n,
				}),
			).toThrowError(Error("Expected value 0 got 1"));
		});

		it("should not throw for correct values", async () => {
			const check = buildFixedParamsCheck({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 1n,
				data: "0x5afe",
				operation: 0,
			});
			check({
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 1n,
				data: "0x5afe",
				operation: 0,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			});
		});
	});
	describe("buildSupportedSelectorCheck", () => {
		it("should throw for data shorter than a selector", async () => {
			const selectors: Hex[] = [];
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSupportedSelectorCheck(selectors, true);
			expect(() => check(tx)).toThrow();
		});

		it("should allow empty data when allowEmpty is true", async () => {
			const selectors: Hex[] = [];
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSupportedSelectorCheck(selectors, true);
			check(tx);
		});

		it("should allow a supported selector", async () => {
			const selectors: Hex[] = ["0x5afe5afe"];
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
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
			const check = buildSupportedSelectorCheck(selectors, true);
			check(tx);
		});
	});

	describe("buildSupportedSignatureCheck", () => {
		it("should throw for data shorter than a selector", async () => {
			const selectors: string[] = [];
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSupportedSignaturesCheck(selectors, true);
			expect(() => check(tx)).toThrow();
		});

		it("should allow empty data when allowEmpty is true", async () => {
			const selectors: string[] = [];
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSupportedSignaturesCheck(selectors, true);
			check(tx);
		});

		it("should allow a supported signature", async () => {
			const selectors: string[] = ["function transfer(address,uint256)"];
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0xa9059cbb",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSupportedSignaturesCheck(selectors, true);
			check(tx);
		});
	});

	describe("buildSelectorChecks", () => {
		it("should throw for data shorter than a selector", async () => {
			const selectors: Record<string, TransactionCheck> = {};
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSelectorChecks(selectors, true);
			expect(() => check(tx)).toThrow("0x5afe is not a valid selector");
		});

		it("should allow empty data when allowEmpty is true", async () => {
			const selectors: Record<string, TransactionCheck> = {};
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSelectorChecks(selectors, true);
			check(tx);
		});

		it("should call sub check", async () => {
			const subCheck = vi.fn();
			const selectors: Record<string, TransactionCheck> = {
				"0xa9059cbb": subCheck,
			};
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0xa9059cbb",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSelectorChecks(selectors, true);
			check(tx);
			expect(subCheck).toBeCalledTimes(1);
			expect(subCheck).toBeCalledWith(tx);
		});

		it("should throw if no check for selector is registered", async () => {
			const subCheck = vi.fn();
			const selectors: Record<string, TransactionCheck> = {
				"0xa9059cbb": subCheck,
			};
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0xa9059cbc",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSelectorChecks(selectors, true);
			expect(() => check(tx)).toThrow("0xa9059cbc not supported");
			expect(subCheck).toBeCalledTimes(0);
		});

		it("should call fallback if no check for selector is registered", async () => {
			const fallbackCheck = vi.fn();
			const subCheck = vi.fn();
			const selectors: Record<string, TransactionCheck> = {
				"0xa9059cbb": subCheck,
			};
			const tx: SafeTransaction = {
				chainId: 1n,
				safe: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0xa9059cbc",
				operation: 1,
				safeTxGas: 0n,
				baseGas: 0n,
				gasPrice: 0n,
				gasToken: zeroAddress,
				refundReceiver: zeroAddress,
				nonce: 0n,
			};
			const check = buildSelectorChecks(selectors, true, fallbackCheck);
			check(tx);
			expect(fallbackCheck).toBeCalledTimes(1);
			expect(fallbackCheck).toBeCalledWith(tx);
			expect(subCheck).toBeCalledTimes(0);
		});
	});
});
