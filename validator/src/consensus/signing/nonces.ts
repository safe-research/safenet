import { numberToBytesBE } from "@noble/curves/utils.js";
import { concatBytes } from "@noble/hashes/utils.js";
import { encodePacked, type Hex, keccak256 } from "viem";
import { h3 } from "../../frost/hashes.js";
import { g, randomBigInt } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import { calculateMerkleRoot } from "../merkle.js";

export type SecretNonceCommitments = {
	hidingNonce: bigint; // d
	bindingNonce: bigint; // e
};

export type PublicNonceCommitments = {
	hidingNonceCommitment: FrostPoint; // D = g(d)
	bindingNonceCommitment: FrostPoint; // E = g(e)
};

export type NonceCommitments = SecretNonceCommitments & PublicNonceCommitments;

export type NonceTree = {
	commitments: NonceCommitments[];
	leaves: Hex[];
	root: Hex;
};

export const generateNonce = (secret: bigint, random: bigint): bigint => {
	return h3(
		concatBytes(numberToBytesBE(random, 32), numberToBytesBE(secret, 32)),
	);
};

export const generateNonceCommitments = (secret: bigint): NonceCommitments => {
	const hidingNonce = generateNonce(secret, randomBigInt());
	const bindingNonce = generateNonce(secret, randomBigInt());
	return {
		hidingNonce,
		bindingNonce,
		hidingNonceCommitment: g(hidingNonce),
		bindingNonceCommitment: g(bindingNonce),
	};
};

const hashNonceCommitments = (c: PublicNonceCommitments): Hex =>
	keccak256(
		encodePacked(
			["uint256", "uint256", "uint256", "uint256"],
			[
				c.hidingNonceCommitment.x,
				c.hidingNonceCommitment.y,
				c.bindingNonceCommitment.x,
				c.bindingNonceCommitment.y,
			],
		),
	);

export const createNonceTree = (
	secret: bigint,
	size: number = 1024,
): NonceTree => {
	const commitments: NonceCommitments[] = [];
	const leaves: Hex[] = [];
	for (let i = 0; i < size; i++) {
		const commitment = generateNonceCommitments(secret);
		commitments.push(commitment);
		leaves.push(hashNonceCommitments(commitment));
	}
	const root = calculateMerkleRoot(leaves);
	return {
		commitments,
		leaves,
		root,
	};
};
