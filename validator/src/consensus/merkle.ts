import { encodePacked, type Hex, keccak256, zeroHash } from "viem";
import type { Participant } from "./types.js";

export const buildMerkleTree = (leaves: Hex[]): Hex[][] => {
	if (leaves.length === 0) throw Error("Cannot generate empty tree!");
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
			const node = keccak256(
				encodePacked(["bytes32", "bytes32"], [left, right]),
			);
			nextLevel.push(node);
		}
		tree.push(nextLevel);
	}
	return tree;
};

export const calculateMerkleRoot = (leaves: Hex[]): Hex => {
	const rootLevel = buildMerkleTree(leaves).at(-1);
	if (rootLevel?.length !== 1) throw Error("Unexpected Merkle Tree");
	return rootLevel[0];
};

export const hashParticipant = (p: Participant): Hex =>
	keccak256(encodePacked(["uint256", "uint256"], [p.index, BigInt(p.address)]));

export const calculateParticipantsRoot = (participants: Participant[]): Hex => {
	// TODO: sort participants
	return calculateMerkleRoot(participants.map(hashParticipant));
};

export const verifyMerkleProof = (
	root: Hex,
	leaf: Hex,
	proof: Hex[],
): boolean => {
	let node: Hex = leaf;
	for (const part of proof) {
		const [left, right] = node < part ? [node, part] : [part, node];
		node = keccak256(encodePacked(["bytes32", "bytes32"], [left, right]));
	}
	return root === node;
};

export const generateMerkleProof = (leaves: Hex[], index: number): Hex[] => {
	const tree = buildMerkleTree(leaves);
	const proof: Hex[] = [];
	const height = tree.length;
	for (let i = 0; i < height - 1; i++) {
		const neighbor = index % 2 === 0 ? index + 1 : index - 1; // index ^ 1
		const node = tree.at(i)?.at(neighbor) ?? zeroHash;
		proof.push(node);
		index = Math.floor(index / 2);
	}
	return proof;
};

export const generateParticipantProof = (
	participants: Participant[],
	index: number,
): Hex[] => {
	// TODO: sort participants
	return generateMerkleProof(participants.map(hashParticipant), index);
};
