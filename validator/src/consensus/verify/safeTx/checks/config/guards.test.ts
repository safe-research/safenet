import { ethAddress } from "viem";
import { describe, expect, it } from "vitest";
import type { MetaTransaction } from "../../schemas.js";
import { buildSetGuardCheck } from "./guards.js";

describe("guards", () => {
	describe("buildSetGuardCheck", () => {
		it("should throw if unsupported data", async () => {
			const tx: MetaTransaction = {
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: "0x5afe",
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			};
			const check = buildSetGuardCheck();
			expect(Object.keys(check)).toStrictEqual(["0xe19a9dd9"]);
			expect(() => check["0xe19a9dd9"](tx)).toThrow();
		});

		it("should throw for unknown guard", async () => {
			const tx: MetaTransaction = {
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: `0xe19a9dd9000000000000000000000000${ethAddress.slice(2)}`,
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			};
			const check = buildSetGuardCheck();
			expect(Object.keys(check)).toStrictEqual(["0xe19a9dd9"]);
			expect(() => check["0xe19a9dd9"](tx)).toThrow(
				"Cannot set unknown guard 0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE",
			);
		});

		it("allow zero address as guard (to reset)", async () => {
			const tx: MetaTransaction = {
				to: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
				value: 0n,
				data: "0xe19a9dd90000000000000000000000000000000000000000000000000000000000000000",
				operation: 0,
				nonce: 0n,
				chainId: 1n,
				account: "0xF01888f0677547Ec07cd16c8680e699c96588E6B",
			};
			const check = buildSetGuardCheck();
			expect(Object.keys(check)).toStrictEqual(["0xe19a9dd9"]);
			check["0xe19a9dd9"](tx);
		});
	});
});
