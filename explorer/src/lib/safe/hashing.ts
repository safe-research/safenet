import { encodeAbiParameters, type Hex, keccak256 } from "viem";
import type { SafeTransaction } from "@/lib/consensus";

// EIP-712 domain separator for Safe transactions
// Type hash for EIP712Domain with only chainId and verifyingContract (no name/version)
const EIP712DOMAIN_TYPEHASH = keccak256("EIP712Domain(uint256 chainId,address verifyingContract)" as Hex);

// EIP-712 SafeTx type hash
const SAFETX_TYPEHASH = keccak256(
	"SafeTx(address to,uint256 value,bytes data,uint8 operation,uint256 safeTxGas,uint256 baseGas,uint256 gasPrice,address gasToken,address refundReceiver,uint256 nonce)" as Hex,
);

export const calculateDomainHash = (chainId: bigint, verifyingContract: string): Hex => {
	// Encode the domain parameters: typeHash, chainId, verifyingContract
	const encoded = encodeAbiParameters(
		[{ type: "bytes32" }, { type: "uint256" }, { type: "address" }],
		[EIP712DOMAIN_TYPEHASH, chainId, verifyingContract as Hex],
	);
	return keccak256(encoded);
};

export const calculateMessageHash = (transaction: SafeTransaction): Hex => {
	// Encode the message: typeHash ++ encodedFields
	// For dynamic types like bytes, we encode them as their keccak256 hash
	const encoded = encodeAbiParameters(
		[
			{ type: "bytes32" }, // typeHash
			{ type: "address" }, // to
			{ type: "uint256" }, // value
			{ type: "bytes32" }, // data (hash)
			{ type: "uint8" }, // operation
			{ type: "uint256" }, // safeTxGas
			{ type: "uint256" }, // baseGas
			{ type: "uint256" }, // gasPrice
			{ type: "address" }, // gasToken
			{ type: "address" }, // refundReceiver
			{ type: "uint256" }, // nonce
		],
		[
			SAFETX_TYPEHASH,
			transaction.to,
			transaction.value,
			keccak256(transaction.data),
			transaction.operation,
			transaction.safeTxGas,
			transaction.baseGas,
			transaction.gasPrice,
			transaction.gasToken,
			transaction.refundReceiver,
			transaction.nonce,
		],
	);
	return keccak256(encoded);
};

export const calculateSafeTxHash = (transaction: SafeTransaction): Hex => {
	const domainHash = calculateDomainHash(transaction.chainId, transaction.safe);
	const messageHash = calculateMessageHash(transaction);

	// EIP-712: final hash is keccak256(0x1901 + domainSeparator + messageHash)
	const prefix = "0x1901";
	const combined = `${prefix}${domainHash.slice(2)}${messageHash.slice(2)}`;
	return keccak256(combined as Hex);
};
