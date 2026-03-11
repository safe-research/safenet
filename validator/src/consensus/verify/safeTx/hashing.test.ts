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

	it("equals safeTxProposalHash with safeTxHash embedded as the safeTxHash field", () => {
		const packetHash = safeTxPacketHash(packet);
		const embeddedHash = safeTxHash(safeTx);
		const proposalHash = safeTxProposalHash({
			domain: packet.domain,
			proposal: { epoch: packet.proposal.epoch, safeTxHash: embeddedHash },
		});
		expect(packetHash).toBe(proposalHash);
	});
});
