import { describe, expect, it } from "vitest";
import { safeTxHash, safeTxPacketHash, safeTxProposalHash } from "./hashing.js";

describe("safeTxPacketHash", () => {
	const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000" as const;
	const TEST_ADDRESS = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" as const;

	const safeTx = {
		chainId: 1n,
		safe: TEST_ADDRESS,
		to: TEST_ADDRESS,
		value: 0n,
		data: "0x" as `0x${string}`,
		operation: 0 as const,
		safeTxGas: 0n,
		baseGas: 0n,
		gasPrice: 0n,
		gasToken: ZERO_ADDRESS,
		refundReceiver: ZERO_ADDRESS,
		nonce: 0n,
	};

	const packet = {
		type: "safe_transaction_packet" as const,
		domain: { chain: 1n, consensus: TEST_ADDRESS },
		proposal: { epoch: 1n, transaction: safeTx },
	};

	const TX_HASH = "0xfe8b85e8d090b16fe8f142d3c9292dc1fc77daf9eb4af8f7cf4a7707d95f4028";
	const PACKET_HASH = "0x3ff98ecae85843603560e9509346df2f35c0ad1dd1ceda5dcbb145745dfc4e00";

	it("equals safeTxProposalHash with safeTxHash embedded as the safeTxHash field", () => {
		expect(
			safeTxProposalHash({
				domain: packet.domain,
				proposal: { epoch: packet.proposal.epoch, safeTxHash: TX_HASH as `0x${string}` },
			}),
		).toBe(PACKET_HASH);
	});
});
