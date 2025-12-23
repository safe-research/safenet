import { describe, expect, it, vi } from "vitest";
import type { TransactionCheck } from "../handler.js";
import type { MetaTransaction } from "../schemas.js";
import { CombinedChecks, AddressSplitCheck } from "./combined.js";

describe("combined checks", () => {
	describe("AddressSplitCheck", () => {
		it("should forward correctly (chain independent)", async () => {
			const check = vi.fn();
			const subCheck = {
				check,
			} as unknown as TransactionCheck;
			const multiSendCheck = new AddressSplitCheck({
				"0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": subCheck,
			});
			multiSendCheck.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
			expect(check).toBeCalledTimes(1);
			expect(check).toBeCalledWith({
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
			const check = vi.fn();
			const subCheck = {
				check,
			} as unknown as TransactionCheck;
			const invalidCheck = {} as unknown as TransactionCheck;
			const multiSendCheck = new AddressSplitCheck({
				"eip155:1:0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": subCheck,
				"0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": invalidCheck,
			});
			multiSendCheck.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
			expect(check).toBeCalledTimes(1);
			expect(check).toBeCalledWith({
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
			const check = vi.fn();
			const fallbackCheck = {
				check
			} as unknown as TransactionCheck;
			const subCheck = {} as unknown as TransactionCheck;
			const multiSendCheck = new AddressSplitCheck({
				"eip155:1:0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": subCheck,
			}, fallbackCheck);
			multiSendCheck.check({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 2n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
			expect(check).toBeCalledTimes(1);
			expect(check).toBeCalledWith({
				to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
				value: 0n,
				data: "0x5afe",
				operation: 1,
				nonce: 0n,
				chainId: 2n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			});
		});

		it("should throw if no check for address is registered", async () => {
			const check = vi.fn();
			const subCheck = {
				check,
			} as unknown as TransactionCheck;
			const multiSendCheck = new AddressSplitCheck({
				"eip155:1:0x40A2aCCbd92BCA938b02010E17A5b8929b49130D": subCheck,
			});
			expect(() =>
				multiSendCheck.check({
					to: "0x40A2aCCbd92BCA938b02010E17A5b8929b49130D",
					value: 0n,
					data: "0x5afe",
					operation: 1,
					nonce: 0n,
					chainId: 2n,
					account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				}),
			).toThrowError(Error("Delegatecalls for eip155:2:0x40A2aCCbd92BCA938b02010E17A5b8929b49130D not allowed!"));
		});
	});

	describe("CombinedChecks", () => {
		it("should throw if any check throws", async () => {
			const check = vi.fn();
			const subCheck = {
				check,
			} as unknown as TransactionCheck;
			const multiSendCheck = new CombinedChecks([subCheck, subCheck, subCheck, subCheck]);
			check.mockImplementationOnce(() => {
				return;
			});
			check.mockImplementationOnce(() => {
				return;
			});
			check.mockImplementationOnce(() => {
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
			expect(() => multiSendCheck.check(tx)).toThrowError(Error("Invalid"));
			expect(check).toBeCalledTimes(3);
			expect(check).toBeCalledWith(tx);
		});

		it("should call all before success", async () => {
			const check = vi.fn();
			const subCheck = {
				check,
			} as unknown as TransactionCheck;
			const multiSendCheck = new CombinedChecks([subCheck, subCheck, subCheck, subCheck]);
			check.mockImplementation(() => {
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
			multiSendCheck.check(tx);
			expect(check).toBeCalledTimes(4);
			expect(check).toBeCalledWith(tx);
		});
	});
});
