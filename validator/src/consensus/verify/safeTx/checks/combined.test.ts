import { describe, expect, it, vi } from "vitest";
import type { TransactionCheck } from "../handler.js";
import type { MetaTransaction } from "../schemas.js";
import { buildAddressSplitCheck, buildCombinedChecks } from "./combined.js";

describe("combined checks", () => {
	describe("buildAddressSplitCheck", () => {
		it("should forward correctly (chain independent)", async () => {
			const subCheck = vi.fn();
			const addressCheck = buildAddressSplitCheck({
				"0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": subCheck,
			});
			addressCheck({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
			expect(subCheck).toBeCalledTimes(1);
			expect(subCheck).toBeCalledWith({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
		});

		it("should forward correctly (chain specific)", async () => {
			const subCheck = vi.fn();
			const invalidCheck = {} as unknown as TransactionCheck;
			const addressCheck = buildAddressSplitCheck({
				"eip155:1:0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": subCheck,
				"0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": invalidCheck,
			});
			addressCheck({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
			expect(subCheck).toBeCalledTimes(1);
			expect(subCheck).toBeCalledWith({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
		});

		it("should call fallback if no check for registered", async () => {
			const subCheck = vi.fn();
			const addressCheck = buildAddressSplitCheck(
				{
					"eip155:1:0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": subCheck,
				},
				subCheck,
			);
			addressCheck({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 2n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
			expect(subCheck).toBeCalledTimes(1);
			expect(subCheck).toBeCalledWith({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 2n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
		});

		it("should pass if no check for address is registered", async () => {
			const subCheck = vi.fn();
			const addressCheck = buildAddressSplitCheck({
				"eip155:1:0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": subCheck,
			});
			addressCheck({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 2n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
		});
	});

	describe("buildCombinedChecks", () => {
		it("should throw if any check throws", async () => {
			const subCheck = vi.fn();
			const combinedCheck = buildCombinedChecks([subCheck, subCheck, subCheck, subCheck]);
			subCheck.mockImplementationOnce(() => {
				return;
			});
			subCheck.mockImplementationOnce(() => {
				return;
			});
			subCheck.mockImplementationOnce(() => {
				throw new Error("Invalid");
			});
			const tx: MetaTransaction = {
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			};
			expect(() => combinedCheck(tx)).toThrowError(Error("Invalid"));
			expect(subCheck).toBeCalledTimes(3);
			expect(subCheck).toBeCalledWith(tx);
		});

		it("should call all before success", async () => {
			const subCheck = vi.fn();
			const combinedCheck = buildCombinedChecks([subCheck, subCheck, subCheck, subCheck]);
			subCheck.mockImplementation(() => {
				return;
			});
			const tx: MetaTransaction = {
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			};
			combinedCheck(tx);
			expect(subCheck).toBeCalledTimes(4);
			expect(subCheck).toBeCalledWith(tx);
		});
	});
});
