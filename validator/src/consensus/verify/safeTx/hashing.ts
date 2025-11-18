import { type Hex, hashTypedData } from "viem";
import type { SafeTransactionPacket } from "./schemas.js";

export const safeTxPacketHash = (packet: SafeTransactionPacket): Hex =>
	hashTypedData({
		domain: {
			verifyingContract: packet.domain.safe,
			chainId: packet.domain.chain,
		},
		types: {
			TransactionProposal: [
				{ type: "address", name: "to" },
				{ type: "uint256", name: "value" },
				{ type: "bytes", name: "data" },
				{ type: "uint8", name: "operation" },
				{ type: "uint256", name: "nonce" },
			],
		},
		primaryType: "TransactionProposal",
		message: {
			...packet.transaction,
		},
	});
