/**
 * Bloom filter inclusion checks.
 *
 * Inspired from the implementation in the Go Ethereum client (`geth`)`:
 * <https://github.com/ethereum/go-ethereum/blob/f3c696fa1db75d0f78ea47dd0975f6f0de6fdd84/core/types/bloom9.go#L86>
 */

import { type Address, type Hex, keccak256, size, toHex } from "viem";

const bitIndex = (bytes: DataView, index: number): number => {
	// Bloom filter is 2048 bits, meaning the bit index is 2 bytes in the range `[0, 2048)`.
	const offset = index * 2;
	return bytes.getUint16(offset) & 0x7ff;
};

const LUT: (number | null)[] = [..."0123456789abcdef"].reduce((lut, c, i) => {
	lut[c.charCodeAt(0) ?? 0] = i;
	lut[c.toUpperCase().charCodeAt(0) ?? 0] = i;
	return lut;
}, Array(256).fill(null));

const isBitSet = (bloom: Hex, index: number): boolean => {
	// Note that we check for bits in a hex string. This means that our `bit` is the position of the
	// bit in a nibble and not a byte.
	const bit = 1 << (index & 3);
	// The offset in the hex string, noting that 513 is the length the bloom hex string (including
	// the `0x` prefix) minus 1.
	const offset = 513 - (index >> 2);

	const c = LUT[bloom.charCodeAt(offset)] ?? 0;
	return (c & bit) === bit;
};

/**
 * Checks whether or not some data is possibly included in a bloom filter.
 *
 * Note that this may have false positives, but will never have false negatives.
 */
export const isInBloom = (bloom: Hex, data: Hex): boolean => {
	if (size(bloom) !== 256) {
		throw new Error("invalid bloom filter");
	}

	const digest = new DataView(keccak256(data, "bytes").buffer);
	return (
		isBitSet(bloom, bitIndex(digest, 0)) && isBitSet(bloom, bitIndex(digest, 1)) && isBitSet(bloom, bitIndex(digest, 2))
	);
};

const addBloom = (bloom: Uint8Array, data: Hex) => {
	const digest = new DataView(keccak256(data, "bytes").buffer);
	for (let i = 0; i < 3; i++) {
		const index = bitIndex(digest, i);
		const bit = 1 << (index & 7);
		const offset = 255 - (index >> 3);
		bloom[offset] |= bit;
	}
};

export type IndexedLogData = {
	address: Address;
	topics: readonly Hex[];
};

/**
 * Computes the bloom filter for the given logs.
 */
export const computeLogsBloom = (logs: readonly IndexedLogData[]): Hex => {
	const bloom = new Uint8Array(256);
	for (const { address, topics } of logs) {
		addBloom(bloom, address);
		for (const topic of topics) {
			addBloom(bloom, topic);
		}
	}
	return toHex(bloom);
};
