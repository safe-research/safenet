import { type Hex, hashStruct, hashTypedData, zeroAddress } from "viem";
import type { MetaTransaction } from "../consensus";
import type { SafeTransaction } from "./service";

const SAFE_TX_TYPE = {
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
};

export const calculateSafeTxHash = (transaction: MetaTransaction): Hex => {
	const domain = {
		chainId: transaction.chainId,
		verifyingContract: transaction.account,
	};

	const message = {
		to: transaction.to,
		value: transaction.value,
		data: transaction.data,
		operation: transaction.operation,
		safeTxGas: 0n,
		baseGas: 0n,
		gasPrice: 0n,
		gasToken: zeroAddress,
		refundReceiver: zeroAddress,
		nonce: transaction.nonce,
	};

	return hashTypedData({
		domain,
		types: SAFE_TX_TYPE,
		primaryType: "SafeTx",
		message,
	});
};

export const metaTxHash = (transaction: SafeTransaction): Hex =>
	hashStruct({
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
		},
		primaryType: "MetaTransaction",
		data: transaction,
	});
