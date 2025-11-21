import { type Hex, hashTypedData } from "viem";
import type { SafeTransactionPacket } from "./schemas.js";

export const safeTxPacketHash = (packet: SafeTransactionPacket): Hex =>
	hashTypedData({
		domain: {
			verifyingContract: packet.domain.consensus,
			chainId: packet.domain.chain,
		},
		types: {
			MetaTransaction: [
				{ type: "uint256", name: "chainId" },
				{ type: "address", name: "account" },
				{ type: "address", name: "to" },
				{ type: "uint256", name: "value" },
				{ type: "uint8", name: "operation" },
				{ type: "bytes", name: "data" },
				{ type: "uint256", name: "nonce" },
			],
			TransactionProposal: [
				{ type: "uint64", name: "epoch" },
				{ type: "MetaTransaction", name: "transaction" },
			],
		},
		primaryType: "TransactionProposal",
		message: {
			...packet.proposal,
		},
	});
