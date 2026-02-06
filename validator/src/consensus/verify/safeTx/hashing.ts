import { type Hex, hashTypedData } from "viem";
import type { SafeTransaction, SafeTransactionPacket } from "./schemas.js";

export const safeTxPacketHash = (packet: SafeTransactionPacket): Hex =>
	hashTypedData({
		domain: {
			chainId: packet.domain.chain,
			verifyingContract: packet.domain.consensus,
		},
		types: {
			TransactionProposal: [
				{ type: "uint64", name: "epoch" },
				{ type: "bytes32", name: "safeTxHash" },
			],
		},
		primaryType: "TransactionProposal",
		message: {
			epoch: packet.proposal.epoch,
			safeTxHash: safeTxHash(packet.proposal.transaction),
		},
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
