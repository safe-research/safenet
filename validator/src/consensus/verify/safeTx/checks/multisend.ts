import { getAddress, size, zeroAddress } from "viem";
import type { TransactionCheck } from "../handler.js";
import type { SafeTransaction } from "../schemas.js";

// selector + pointer + length
const MIN_LENGTH = 4 + 32 + 32;

const MULTI_SEND_SELECTOR = "0x8d80ff0a";

// MultiSend data should always start with specific data pointer (0x20)
const MULTI_SEND_DATA_START = `${MULTI_SEND_SELECTOR}0000000000000000000000000000000000000000000000000000000000000020`;

const decodeMultiSend = ({ chainId, safe, data, nonce }: SafeTransaction): SafeTransaction[] => {
	const dataSize = size(data);
	if (dataSize < MIN_LENGTH) {
		throw new Error("Invalid multi send encoding");
	}
	if (!data.startsWith(MULTI_SEND_DATA_START)) {
		throw new Error("Invalid multi send prefix");
	}
	let pointer = MULTI_SEND_DATA_START.length;
	// Read total data length as bigint
	const multiSendDataLength = BigInt(`0x${data.slice(pointer, pointer + 64)}`);
	pointer += 64;
	// Calculate data padding that is appended by default abi encoders
	const multiSendDataPadding = (32n - (multiSendDataLength % 32n)) % 32n;

	if (multiSendDataLength + multiSendDataPadding !== BigInt(dataSize - MIN_LENGTH)) {
		throw new Error("Invalid multi send data length");
	}

	const txs: SafeTransaction[] = [];
	while (BigInt(pointer) / 2n + multiSendDataPadding < BigInt(dataSize)) {
		// Read 1 byte for the operation as number
		const operation = Number(`0x${data.slice(pointer, pointer + 2)}`);
		if (operation !== 0 && operation !== 1) throw new Error(`Invalid MultiSend operation ${operation}`);
		pointer += 2;
		// Read 20 bytes for to as an address
		const to = getAddress(`0x${data.slice(pointer, pointer + 40)}`);
		pointer += 40;
		// Read 64 bytes for the value as a bigint
		const value = BigInt(`0x${data.slice(pointer, pointer + 64)}`);
		pointer += 64;
		// Read 64 bytes for the data length as a number
		const subDataLength = Number(`0x${data.slice(pointer, pointer + 64)}`);
		pointer += 64;
		// Read the data as a hex string. subDataLength is in bytes, multiply by 2 for string position
		const subData = `0x${data.slice(pointer, pointer + subDataLength * 2)}` as const;
		pointer += subDataLength * 2;
		txs.push({
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
	// The pointer includes the hex prefix (0x) therefore we have to subtract 1 byte before comparing
	if (BigInt(pointer / 2 - MIN_LENGTH - 1) !== multiSendDataLength) {
		throw new Error("Unexpected pointer position after decoding");
	}
	return txs;
};

export const buildMultiSendCallOnlyCheck =
	(check: TransactionCheck): TransactionCheck =>
	(tx: SafeTransaction) => {
		if (tx.operation !== 1) throw new Error("MultiSend has to be performed with delegatecall");
		if (tx.value !== 0n) throw new Error("MultiSend should not be executed with value");

		const subTxs: SafeTransaction[] = decodeMultiSend(tx);

		for (const tx of subTxs) {
			check(tx);
		}
	};
