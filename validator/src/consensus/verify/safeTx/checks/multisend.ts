import { decodeFunctionData, getAddress, type Hex, parseAbiItem, size, slice, zeroAddress } from "viem";
import type { TransactionCheck } from "../handler.js";
import type { SafeTransaction } from "../schemas.js";
import { buildFixedParamsCheck } from "./basic.js";
import { TransactionCheckError } from "./errors.js";

const MULTI_SEND = parseAbiItem("function multiSend(bytes transactions)");
const TRANSACTION_FIXED_SIZE = 1 + 20 + 32 + 32;

const decodeMultiSendFunctionData = ({ data }: Pick<SafeTransaction, "data">): Hex => {
	try {
		const {
			args: [transactions],
		} = decodeFunctionData({
			abi: [MULTI_SEND],
			data,
		});
		return transactions;
	} catch (cause) {
		throw new TransactionCheckError("invalid_multisend", "Invalid multi send transaction ABI encoding", { cause });
	}
};

const decodeMultiSend = ({
	chainId,
	safe,
	data,
	nonce,
}: Pick<SafeTransaction, "chainId" | "safe" | "data" | "nonce">): SafeTransaction[] => {
	const txs = decodeMultiSendFunctionData({ data });
	const result: SafeTransaction[] = [];
	let pointer = 0;
	while (pointer + TRANSACTION_FIXED_SIZE <= size(txs)) {
		// Read 1 byte for the operation as number
		const operation = Number(slice(txs, pointer, pointer + 1));
		if (operation !== 0 && operation !== 1) {
			throw new TransactionCheckError("invalid_multisend", `Invalid MultiSend operation ${operation}`);
		}
		pointer += 1;
		// Read 20 bytes for to as an address
		const to = getAddress(slice(txs, pointer, pointer + 20));
		pointer += 20;
		// Read 32 bytes for the value as a bigint
		const value = BigInt(slice(txs, pointer, pointer + 32));
		pointer += 32;
		// Read 32 bytes for the sub-data length as a number
		const subDataLength = Number(slice(txs, pointer, pointer + 32));
		pointer += 32;
		// Read the sub-data bytes
		const subData = slice(txs, pointer, pointer + subDataLength);
		pointer += subDataLength;
		result.push({
			chainId,
			safe,
			to,
			value,
			data: subData,
			operation,
			// Meta transactions do not contain these fields, so we use synthetic values that approximate how the
			// execution actually happens on-chain.
			safeTxGas: 0n,
			baseGas: 0n,
			gasPrice: 0n,
			gasToken: zeroAddress,
			refundReceiver: zeroAddress,
			nonce,
		});
	}
	// Check that the pointer is exactly at the end of the transactions. This is not the case if:
	// - We reach a point where there are fewer than `TRANSACTION_FIXED_SIZE` bytes remaining, but
	//   we did not reach the end of the transaction bytes.
	if (pointer !== size(txs)) {
		throw new TransactionCheckError("invalid_multisend", "Invalid MultiSend transaction encoding");
	}
	return result;
};

export const buildMultiSendCallOnlyCheck = (check: TransactionCheck): TransactionCheck => {
	const fixed = buildFixedParamsCheck("invalid_multisend", { operation: 1, value: 0n });
	return (tx: SafeTransaction) => {
		fixed(tx);
		for (const subTx of decodeMultiSend(tx)) {
			check(subTx);
		}
	};
};
