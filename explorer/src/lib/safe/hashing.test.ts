import { zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import { calculateSafeTxHash, metaTxHash } from "./hashing";

describe("hashing", () => {
	describe("Safe transaction hash", () => {
		it("should return correct hash", () => {
			expect(
				calculateSafeTxHash({
					account: "0x779720809250AF7931935a192FCD007479C41299",
					chainId: 100n,
					to: "0x2dC63c83040669F0aDBa5F832F713152bA862c97",
					data: "0x",
					value: 100000000000000000n,
					operation: 0,
					nonce: 1n,
				}),
			).toBe("0xd6a2395bd7bd650df56610d38760d1b4b8073d37db35090ce3c855ef659c1b81");
		});
	});

	describe("Meta transaction hash", () => {
		it("should return correct hash", () => {
			expect(
				metaTxHash({
					account: "0x779720809250AF7931935a192FCD007479C41299",
					safe: "0x779720809250AF7931935a192FCD007479C41299",
					chainId: 100n,
					to: "0xF71d416606B484E7EBCb9830046A3E8F16613F37",
					data: "0x5afe5afe",
					value: 0n,
					operation: 0,
					nonce: 7n,
					safeTxHash: "0xde599c0fa706eb7da4fbbf2d2f2a4c9eaa521d817f098308577be28b5c0a90cc",
					safeTxGas: 0n,
					baseGas: 0n,
					gasPrice: 0n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
				}),
			).toBe("0x3dfc478b3db970afa32c4cbc254769932f1ae0d527ad68ee58716538c8bb6783");
		});
	});
});
