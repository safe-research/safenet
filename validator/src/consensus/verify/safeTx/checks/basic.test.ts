import { describe, expect, it } from "vitest";
import { FixedParamsCheck, NoDelegateCallCheck } from "./basic.js";
import { ethAddress } from "viem";

describe("basic checks", () => {
	describe("NoDelegateCallCheck", () => {
		it("should throw for delegatecalls", async () => {
			const check = new NoDelegateCallCheck();
			expect(() => check.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			})).toThrowError(Error("Delegatecall not allowed"));
		});

		it("should forward continue for call transactions", async () => {
			const check = new NoDelegateCallCheck();
			check.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			})
		});
	});
	describe("Fixed Params", () => {
		it("should throw for wrong operation", async () => {
			const check = new FixedParamsCheck({
				operation: 0
			});
			expect(() => check.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			})).toThrowError(Error("Expected operation 0 got 1"));
		});

		it("should throw for wrong to", async () => {
			const check = new FixedParamsCheck({
				to: ethAddress
			});
			expect(() => check.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			})).toThrowError(Error(`Expected to ${ethAddress} got 0x40A2aCCbd92BCA938b02010E17A5b8929b49130D`));
		});

		it("should throw for wrong data", async () => {
			const check = new FixedParamsCheck({
				data: "0x5afe5afe"
			});
			expect(() => check.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			})).toThrowError(Error(`Expected data 0x5afe5afe got 0x5afe`));
		});

		it("should throw for wrong value", async () => {
			const check = new FixedParamsCheck({
				value: 0n
			});
			expect(() => check.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 1n,
				data: "0x5afe",
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			})).toThrowError(Error(`Expected value 0 got 1`));
		});

		it("should not throw for correct values", async () => {
			const check = new FixedParamsCheck({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 1n,
				data: "0x5afe",
				operation: 0,
			});
			check.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 1n,
				data: "0x5afe",
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			})
		});
	});
});
