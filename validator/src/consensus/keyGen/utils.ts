import { encodePacked, type Hex, keccak256, numberToHex } from "viem";

export const calcGroupId = (participantsRoot: Hex, count: bigint, threshold: bigint, context: Hex): Hex => {
	const infoHash = BigInt(
		keccak256(
			encodePacked(["bytes32", "uint256", "uint256", "bytes32"], [participantsRoot, count, threshold, context]),
		),
	);
	return numberToHex(infoHash & 0xffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000n, { size: 32 });
};
