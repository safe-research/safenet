import { randomBytes } from "node:crypto";
import { concatBytes } from "@noble/curves/utils.js";
import { encodePacked, type Hex, hexToBytes, keccak256 } from "viem";
import { h1, h3, h4, h5 } from "../../frost/hashes.js";
import { g, scalarToBytes } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import { calculateMerkleRoot, generateMerkleProof } from "../merkle.js";

const SEQUENCE_CHUNK_SIZE = 1024n;

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

export const generateNonce = (
	secret: bigint,
	randomness?: Uint8Array,
): bigint => {
	const random = randomness ?? randomBytes(32);
	if (random.length !== 32) {
		throw new Error("invalid nonce randomness");
	}
	return h3(concatBytes(random, scalarToBytes(secret)));
};

export const generateNonceCommitments = (secret: bigint): NonceCommitments => {
	const hidingNonce = generateNonce(secret);
	const bindingNonce = generateNonce(secret);
	return {
		hidingNonce,
		bindingNonce,
		hidingNonceCommitment: g(hidingNonce),
		bindingNonceCommitment: g(bindingNonce),
	};
};

const hashNonceCommitments = (id: bigint, c: PublicNonceCommitments): Hex =>
	keccak256(
		encodePacked(
			["uint256", "uint256", "uint256", "uint256", "uint256"],
			[
				id,
				c.hidingNonceCommitment.x,
				c.hidingNonceCommitment.y,
				c.bindingNonceCommitment.x,
				c.bindingNonceCommitment.y,
			],
		),
	);

export const createNonceTree = (
	secret: bigint,
	size: bigint = SEQUENCE_CHUNK_SIZE,
): NonceTree => {
	const commitments: NonceCommitments[] = [];
	const leaves: Hex[] = [];
	for (let i = 0n; i < size; i++) {
		const commitment = generateNonceCommitments(secret);
		commitments.push(commitment);
		leaves.push(hashNonceCommitments(i, commitment));
	}
	const root = calculateMerkleRoot(leaves);
	return {
		commitments,
		leaves,
		root,
	};
};

const encodeCommitments = (
	signers: bigint[],
	nonceCommitments: Map<bigint, PublicNonceCommitments>,
): Uint8Array => {
	return concatBytes(
		...signers.map((id) => {
			const commitments = nonceCommitments.get(id);
			if (commitments === undefined)
				throw Error(`Missing nonce commitments for ${id}`);
			return concatBytes(
				scalarToBytes(id),
				commitments.hidingNonceCommitment.toBytes(true),
				commitments.bindingNonceCommitment.toBytes(true),
			);
		}),
	);
};

export type BindingFactor = {
	id: bigint;
	bindingFactor: bigint;
};

export const bindingPrefix = (
	groupPublicKey: FrostPoint,
	signers: bigint[],
	nonceCommitments: Map<bigint, PublicNonceCommitments>,
	message: Hex,
): Uint8Array => {
	const serializedKey = groupPublicKey.toBytes(true);
	const msgHash = h4(hexToBytes(message));
	const commitmentHash = h5(encodeCommitments(signers, nonceCommitments));
	return concatBytes(serializedKey, msgHash, commitmentHash);
};

export const bindingFactor = (
	signerId: bigint,
	bindingPrefix: Uint8Array,
): bigint => {
	return h1(concatBytes(bindingPrefix, scalarToBytes(signerId)));
};

export const bindingFactors = (
	groupPublicKey: FrostPoint,
	signers: bigint[],
	nonceCommitments: Map<bigint, PublicNonceCommitments>,
	message: Hex,
): BindingFactor[] => {
	const prefix = bindingPrefix(
		groupPublicKey,
		signers,
		nonceCommitments,
		message,
	);
	return signers.map((id) => {
		return {
			id,
			bindingFactor: bindingFactor(id, prefix),
		};
	});
};

export const groupCommitmentShare = (
	bindingFactor: bigint,
	nonceCommitments: PublicNonceCommitments,
): FrostPoint => {
	const factor =
		nonceCommitments.bindingNonceCommitment.multiply(bindingFactor);
	return nonceCommitments.hidingNonceCommitment.add(factor);
};

export const groupCommitementShares = (
	bindingFactors: BindingFactor[],
	nonceCommitments: Map<bigint, PublicNonceCommitments>,
): FrostPoint[] => {
	return bindingFactors.map((bf) => {
		const commitments = nonceCommitments.get(bf.id);
		if (commitments === undefined)
			throw Error(`Missing nonce commitments for ${bf.id}`);
		const factor = commitments.bindingNonceCommitment.multiply(
			bf.bindingFactor,
		);
		return commitments.hidingNonceCommitment.add(factor);
	});
};

export const calculateGroupCommitment = (
	groupCommitmentShares: FrostPoint[],
): FrostPoint => {
	return groupCommitmentShares.reduce((v, c) => v.add(c));
};

export const decodeSequence = (
	sequence: bigint,
	chunkSize: bigint = SEQUENCE_CHUNK_SIZE,
): {
	chunk: bigint;
	offset: bigint;
} => {
	const chunk = sequence / chunkSize;
	const offset = sequence % chunkSize;
	return {
		chunk,
		offset,
	};
};

export const nonceCommitmentsWithProof = (
	nonceTree: NonceTree,
	offset: bigint,
): {
	nonceCommitments: NonceCommitments;
	nonceProof: Hex[];
} => {
	const nonceOffset = Number(offset);
	const nonceCommitments = nonceTree.commitments[nonceOffset];
	const nonceProof = generateMerkleProof(nonceTree.leaves, nonceOffset);
	return {
		nonceCommitments,
		nonceProof,
	};
};
