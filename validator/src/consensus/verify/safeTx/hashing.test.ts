import { describe, expect, it } from "vitest";
import { safeTxHash, safeTxPacketHash, safeTxProposalHash, safeTxStructHash } from "./hashing.js";

const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000" as const;
const TEST_ADDRESS = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" as const;
const OTHER_ADDRESS = "0x71C7656EC7ab88b098defB751B7401B5f6d8976F" as const;

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

describe("safeTxHash", () => {
	it("deterministic: same transaction → same hash", () => {
		const hash1 = safeTxHash(safeTx);
		const hash2 = safeTxHash({ ...safeTx });
		expect(hash1).toBe(hash2);
	});

	it("different chainId → different hash", () => {
		const hash1 = safeTxHash(safeTx);
		const hash2 = safeTxHash({ ...safeTx, chainId: 2n });
		expect(hash1).not.toBe(hash2);
	});

	it("different safe address → different hash", () => {
		const hash1 = safeTxHash(safeTx);
		const hash2 = safeTxHash({ ...safeTx, safe: OTHER_ADDRESS });
		expect(hash1).not.toBe(hash2);
	});

	it("returns a 0x-prefixed hex string of length 66 (bytes32)", () => {
		const hash = safeTxHash(safeTx);
		expect(hash).toMatch(/^0x[0-9a-f]{64}$/i);
		expect(hash.length).toBe(66);
	});
});

describe("safeTxStructHash", () => {
	const { chainId: _chainId, safe: _safe, ...txData } = safeTx;

	it("deterministic: same struct data → same hash", () => {
		const hash1 = safeTxStructHash(txData);
		const hash2 = safeTxStructHash({ ...txData });
		expect(hash1).toBe(hash2);
	});

	it("differs from safeTxHash (no domain)", () => {
		const structHash = safeTxStructHash(txData);
		const txHash = safeTxHash(safeTx);
		expect(structHash).not.toBe(txHash);
	});

	it("different nonce → different hash", () => {
		const hash1 = safeTxStructHash(txData);
		const hash2 = safeTxStructHash({ ...txData, nonce: 1n });
		expect(hash1).not.toBe(hash2);
	});
});

describe("safeTxProposalHash", () => {
	const domain = { chain: 1n, consensus: TEST_ADDRESS };
	const proposal = {
		epoch: 1n,
		safeTxHash: "0x0000000000000000000000000000000000000000000000000000000000000001" as `0x${string}`,
	};

	it("deterministic: same proposal → same hash", () => {
		const hash1 = safeTxProposalHash({ domain, proposal });
		const hash2 = safeTxProposalHash({ domain: { ...domain }, proposal: { ...proposal } });
		expect(hash1).toBe(hash2);
	});

	it("different epoch → different hash", () => {
		const hash1 = safeTxProposalHash({ domain, proposal });
		const hash2 = safeTxProposalHash({ domain, proposal: { ...proposal, epoch: 2n } });
		expect(hash1).not.toBe(hash2);
	});

	it("different consensus address → different hash", () => {
		const hash1 = safeTxProposalHash({ domain, proposal });
		const hash2 = safeTxProposalHash({ domain: { ...domain, consensus: OTHER_ADDRESS }, proposal });
		expect(hash1).not.toBe(hash2);
	});
});

describe("safeTxPacketHash", () => {
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

	it("deterministic", () => {
		const hash1 = safeTxPacketHash(packet);
		const hash2 = safeTxPacketHash({ ...packet });
		expect(hash1).toBe(hash2);
	});

	it("changing the transaction changes the packet hash", () => {
		const hash1 = safeTxPacketHash(packet);
		const hash2 = safeTxPacketHash({
			...packet,
			proposal: { ...packet.proposal, transaction: { ...safeTx, nonce: 99n } },
		});
		expect(hash1).not.toBe(hash2);
	});
});
