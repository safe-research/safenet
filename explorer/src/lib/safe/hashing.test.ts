import { zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import type { SafeTransaction } from "../consensus";
import { calculateDomainHash, calculateMessageHash, calculateSafeTxHash } from "./hashing";

describe("hashing", () => {
	const transaction: SafeTransaction = {
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
	};

	const expectedSafeTxHash = "0xd6a2395bd7bd650df56610d38760d1b4b8073d37db35090ce3c855ef659c1b81";
	const expectedDomainHash = "0x82bc8380f75c44eca16d1e557c5f25b9a93076a556d074f2410440b073f61c60";
	const expectedMessageHash = "0xe5e1f9014523ee27d09c7c545d66f53e835ce1fc3e140cea79547d909c65ded0";

	describe("Safe transaction hash", () => {
		it("should return correct hash", () => {
			expect(calculateSafeTxHash(transaction)).toBe(expectedSafeTxHash);
		});
	});

	describe("Domain hash", () => {
		it("should return correct domain hash", () => {
			expect(calculateDomainHash(transaction.chainId, transaction.safe)).toBe(expectedDomainHash);
		});
	});

	describe("Message hash", () => {
		it("should return correct message hash", () => {
			expect(calculateMessageHash(transaction)).toBe(expectedMessageHash);
		});
	});
});
