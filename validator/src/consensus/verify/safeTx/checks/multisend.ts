import { decodeFunctionData, getAddress, type Hex, parseAbiItem, size, slice, zeroAddress } from "viem";
import type { TransactionCheck } from "../handler.js";
import type { SafeTransaction } from "../schemas.js";
import { buildFixedParamsCheck } from "./basic.js";
import { TransactionCheckError } from "./errors.js";

const MULTI_SEND = parseAbiItem("function multiSend(bytes transactions)");

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

const checkedSlice = (data: Hex, offset: number, len: number): Hex => {
	// Viem doesn't like reading 0-length slices at the end of bytes, so handle
	// that case manually.
	if (len === 0 && offset <= size(data)) {
		return "0x";
	}
	try {
		return slice(data, offset, offset + len, { strict: true });
	} catch {
		throw new TransactionCheckError("invalid_multisend", "Invalid MultiSend transaction encoding");
	}
};

const decodeMultiSend = (
	{ chainId, safe, data, nonce }: Pick<SafeTransaction, "chainId" | "safe" | "data" | "nonce">,
	options: MultiSendOptions,
): SafeTransaction[] => {
	const txs = decodeMultiSendFunctionData({ data });
	const result: SafeTransaction[] = [];
	let pointer = 0;
	while (pointer < size(txs)) {
		// Read 1 byte for the operation as number
		const operation = Number(checkedSlice(txs, pointer, 1));
		if (operation !== 0 && operation !== 1) {
			throw new TransactionCheckError("invalid_multisend", `Invalid MultiSend operation ${operation}`);
		}
		pointer += 1;
		// Read 20 bytes for to as an address
		let to = getAddress(checkedSlice(txs, pointer, 20));
		if (options.toZeroIsSelf === true && to === zeroAddress) {
			// In recent versions of the Safe contracts (v1.5.0+), setting the `to` to be the zero address
			// causes the contract to call itself.
			to = safe;
		}
		pointer += 20;
		// Read 32 bytes for the value as a bigint
		const value = BigInt(checkedSlice(txs, pointer, 32));
		pointer += 32;
		// Read 32 bytes for the sub-data length as a number
		const subDataLength = Number(checkedSlice(txs, pointer, 32));
		pointer += 32;
		// Read the sub-data bytes
		const subData = checkedSlice(txs, pointer, subDataLength);
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

	return result;
};

export type MultiSendOptions = {
	toZeroIsSelf?: boolean;
};

export const buildMultiSendCallOnlyCheck = (
	check: TransactionCheck,
	options: MultiSendOptions = {},
): TransactionCheck => {
	const fixed = buildFixedParamsCheck("invalid_multisend", { operation: 1 });
	return (tx: SafeTransaction) => {
		fixed(tx);
		for (const subTx of decodeMultiSend(tx, options)) {
			check(subTx);
		}
	};
};
