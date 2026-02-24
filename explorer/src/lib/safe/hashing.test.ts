import { zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import { calculateSafeTxHash } from "./hashing";

describe("hashing", () => {
	describe("Safe transaction hash", () => {
		it("should return correct hash", () => {
			expect(
				calculateSafeTxHash({
					chainId: 100n,
					safe: "0x779720809250AF7931935a192FCD007479C41299",
					to: "0x2dC63c83040669F0aDBa5F832F713152bA862c97",
					data: "0x",
					value: 100000000000000000n,
					operation: 0,
					safeTxGas: 0n,
					baseGas: 0n,
					gasPrice: 0n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
					nonce: 1n,
				}),
			).toBe("0xd6a2395bd7bd650df56610d38760d1b4b8073d37db35090ce3c855ef659c1b81");
		});
	});
});
