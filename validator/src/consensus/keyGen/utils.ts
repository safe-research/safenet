import { encodeAbiParameters, type Hex, keccak256, numberToHex, parseAbiParameters } from "viem";

export const calcGroupId = (participantsRoot: Hex, count: number, threshold: number, context: Hex): Hex => {
	const infoHash = BigInt(
		keccak256(
			encodeAbiParameters(parseAbiParameters("bytes32, uint16, uint16, bytes32"), [
				participantsRoot,
				count,
				threshold,
				context,
			]),
		),
	);
	return numberToHex(infoHash & 0xffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000n, { size: 32 });
};
