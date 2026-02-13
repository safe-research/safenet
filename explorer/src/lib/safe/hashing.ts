import { type Hex, hashTypedData } from "viem";
import type { SafeTransaction } from "../consensus";

export const calculateSafeTxHash = (transaction: SafeTransaction): Hex => {
	const domain = {
		chainId: transaction.chainId,
		verifyingContract: transaction.safe,
	};

	return hashTypedData({
		domain,
		types: {
			SafeTx: [
				{ name: "to", type: "address" },
				{ name: "value", type: "uint256" },
				{ name: "data", type: "bytes" },
				{ name: "operation", type: "uint8" },
				{ name: "safeTxGas", type: "uint256" },
				{ name: "baseGas", type: "uint256" },
				{ name: "gasPrice", type: "uint256" },
				{ name: "gasToken", type: "address" },
				{ name: "refundReceiver", type: "address" },
				{ name: "nonce", type: "uint256" },
			],
		},
		primaryType: "SafeTx",
		message: transaction,
	});
};
