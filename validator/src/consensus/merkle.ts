import { type Address, encodePacked, getAddress, type Hex, keccak256, pad, slice, zeroHash } from "viem";

export const buildMerkleTree = (leaves: Hex[]): Hex[][] => {
	if (leaves.length === 0) throw new Error("Cannot generate empty tree!");
	const tree: Hex[][] = [];
	tree.push(leaves);
	while (tree[tree.length - 1].length > 1) {
		const nextLevel: Hex[] = [];
		const currentLevel = tree[tree.length - 1];
		const currentOrder = Math.ceil(currentLevel.length / 2);
		for (let i = 0; i < currentOrder; i++) {
			const a = currentLevel.at(i * 2) ?? zeroHash;
			const b = currentLevel.at(i * 2 + 1) ?? zeroHash;
			const [left, right] = a < b ? [a, b] : [b, a];
			const node = keccak256(encodePacked(["bytes32", "bytes32"], [left, right]));
			nextLevel.push(node);
		}
		tree.push(nextLevel);
	}
	return tree;
};

export const calculateMerkleRoot = (leaves: Hex[]): Hex => {
	const rootLevel = buildMerkleTree(leaves).at(-1);
	if (rootLevel?.length !== 1) throw new Error("Unexpected Merkle Tree");
	return rootLevel[0];
};

export const verifyMerkleProof = (root: Hex, leaf: Hex, proof: Hex[]): boolean => {
	let node: Hex = leaf;
	for (const part of proof) {
		const [left, right] = node < part ? [node, part] : [part, node];
		node = keccak256(encodePacked(["bytes32", "bytes32"], [left, right]));
	}
	return root === node;
};

export const generateMerkleProofWithRoot = (
	leaves: Hex[],
	index: number,
): {
	proof: Hex[];
	root: Hex;
} => {
	const tree = buildMerkleTree(leaves);
	const proof: Hex[] = [];
	const height = tree.length;
	let currentIndex = index;
	for (let i = 0; i < height - 1; i++) {
		const neighbor = currentIndex % 2 === 0 ? currentIndex + 1 : currentIndex - 1; // currentIndex ^ 1
		const node = tree.at(i)?.at(neighbor) ?? zeroHash;
		proof.push(node);
		currentIndex = Math.floor(currentIndex / 2); // currentIndex >> 1
	}
	return {
		proof,
		root: tree[height - 1][0],
	};
};

export const generateMerkleProof = (leaves: Hex[], index: number): Hex[] => {
	const { proof } = generateMerkleProofWithRoot(leaves, index);
	return proof;
};

const sortedParticipantLeaves = (participants: readonly Address[]): Hex[] => {
	// In order to ensure stable participation roots, we make sure that we sort the addresses
	// lexographically (i.e. based on their value and not checksummed string representation)
	// before building our participant Merkle tree.
	const sorted = participants.map((p) => pad(p).toLowerCase() as Hex).sort();

	// We cannot support duplicate participants (they would have the same ID). This should be
	// impossible to happen, so assert that it is true. Take advantage that the array is sorted
	// and just check that no two consecutive items are equal.
	sorted.reduce((previous, leaf) => {
		if (previous === leaf) {
			throw new Error(`duplicate participant ${getAddress(slice(leaf, 12))}`);
		}
		return leaf;
	});

	return sorted;
};

export const calculateParticipantsRoot = (participants: readonly Address[]): Hex => {
	return calculateMerkleRoot(sortedParticipantLeaves(participants));
};

export const generateParticipantProof = (participants: readonly Address[], participant: Address): Hex[] => {
	const leaves = sortedParticipantLeaves(participants);
	const index = leaves.findIndex((p) => BigInt(p) === BigInt(participant));
	if (index < 0) {
		throw new Error(`participant ${participant} not included`);
	}
	return generateMerkleProof(leaves, index);
};
