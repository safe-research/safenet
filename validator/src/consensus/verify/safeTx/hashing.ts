import { type Hex, hashTypedData } from "viem";
import type { SafeTransaction, SafeTransactionPacket } from "./schemas.js";

export type SafeTransactionProposal = {
	domain: SafeTransactionPacket["domain"];
	proposal: {
		epoch: bigint;
		safeTxHash: Hex;
	};
};

export const safeTxProposalHash = ({ domain, proposal }: SafeTransactionProposal) =>
	hashTypedData({
		domain: {
			chainId: domain.chain,
			verifyingContract: domain.consensus,
		},
		types: {
			TransactionProposal: [
				{ type: "uint64", name: "epoch" },
				{ type: "bytes32", name: "safeTxHash" },
			],
		},
		primaryType: "TransactionProposal",
		message: proposal,
	});

export const safeTxPacketHash = (packet: SafeTransactionPacket): Hex =>
	safeTxProposalHash({
		domain: packet.domain,
		proposal: { epoch: packet.proposal.epoch, safeTxHash: safeTxHash(packet.proposal.transaction) },
	});

export const safeTxHash = (transaction: SafeTransaction): Hex =>
	hashTypedData({
		domain: {
			chainId: transaction.chainId,
			verifyingContract: transaction.safe,
		},
		types: {
			SafeTx: [
				{ type: "address", name: "to" },
				{ type: "uint256", name: "value" },
				{ type: "bytes", name: "data" },
				{ type: "uint8", name: "operation" },
				{ type: "uint256", name: "safeTxGas" },
				{ type: "uint256", name: "baseGas" },
				{ type: "uint256", name: "gasPrice" },
				{ type: "address", name: "gasToken" },
				{ type: "address", name: "refundReceiver" },
				{ type: "uint256", name: "nonce" },
			],
		},
		primaryType: "SafeTx",
		message: transaction,
	});
