import { type Address, type Hex, hashDomain, hashStruct, hashTypedData } from "viem";
import type { SafeTransaction } from "@/lib/consensus";

// EIP-712 domain separator for Safe transactions
// Use viem's hashDomain to compute the domain hash
export const calculateDomainHash = (chainId: bigint, verifyingContract: Address): Hex => {
	return hashDomain({
		domain: {
			chainId,
			verifyingContract,
		},
		types: {
			EIP712Domain: [
				{ name: "chainId", type: "uint256" },
				{ name: "verifyingContract", type: "address" },
			],
		},
	});
};

// EIP-712 SafeTx types - extracted for reuse
const SAFETX_TYPES = {
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
} as const;

// EIP-712 SafeTx message hash
// Use viem's hashStruct to compute the message hash (struct hash without domain)
export const calculateMessageHash = (transaction: SafeTransaction): Hex => {
	return hashStruct({
		data: transaction,
		primaryType: "SafeTx",
		types: SAFETX_TYPES,
	});
};

export const calculateSafeTxHash = (transaction: SafeTransaction): Hex => {
	const domain = {
		chainId: transaction.chainId,
		verifyingContract: transaction.safe,
	};

	// Use viem's hashTypedData for the full EIP-712 hash
	return hashTypedData({
		domain,
		types: SAFETX_TYPES,
		primaryType: "SafeTx",
		message: transaction,
	});
};
