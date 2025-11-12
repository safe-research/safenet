import { randomBytes } from "node:crypto";
import { mod } from "@noble/curves/abstract/modular.js";
import { secp256k1 } from "@noble/curves/secp256k1.js";
import {
	bytesToNumberBE,
	concatBytes,
	numberToBytesBE,
} from "@noble/curves/utils.js";
import { keccak_256 } from "@noble/hashes/sha3.js";
import { hexToBigInt } from "viem";
import type { FrostPoint, GroupId } from "./types.js";

const G_BASE = secp256k1.Point.BASE;
const N = secp256k1.Point.CURVE().n;

export const randomBigInt = () => bytesToNumberBE(randomBytes(32));

export const g = (scalar: bigint): FrostPoint => {
	const point = G_BASE.multiply(scalar);
	point.assertValidity();
	return point;
};

export const mod_n = (x: bigint) => {
	return mod(x, N);
};

export const toPoint = (coordinates: { x: bigint; y: bigint }): FrostPoint => {
	const point = secp256k1.Point.fromAffine(coordinates);
	point.assertValidity();
	return point;
};

// TODO: replace by proper hashing function
export const hashToBigInt = (
	index: bigint,
	ga0: FrostPoint,
	r: FrostPoint,
	groupId: GroupId,
): bigint => {
	const indexBytes = numberToBytesBE(index, 32);
	const grouIdBytes = numberToBytesBE(hexToBigInt(groupId), 32);
	const ga0xBytes = numberToBytesBE(ga0.x, 32);
	const ga0yBytes = numberToBytesBE(ga0.y, 32);
	const rxBytes = numberToBytesBE(r.x, 32);
	const ryBytes = numberToBytesBE(r.y, 32);

	// Concatenate all bytes as you specified
	const allBytes = concatBytes(
		indexBytes,
		grouIdBytes,
		ga0xBytes,
		ga0yBytes,
		rxBytes,
		ryBytes,
	);
	const hash = keccak_256(allBytes); // 32-byte array

	// Convert the hash digest to a bigint
	const c = bytesToNumberBE(hash);

	// CRITICAL: Reduce the bigint modulo the curve order n
	return mod_n(c);
};

export const createVerificationShare = (
	allCommitments: Map<bigint, readonly FrostPoint[]>,
	senderIndex: bigint,
): FrostPoint => {
	let verificationShare = null;
	for (const [, commitments] of allCommitments) {
		const partialVerificationShare = evalCommitment(commitments, senderIndex);
		verificationShare =
			verificationShare == null
				? partialVerificationShare
				: verificationShare.add(partialVerificationShare);
	}
	if (verificationShare === null)
		throw Error("Could not calculate verification share!");
	return verificationShare;
};

export const createSigningShare = (
	secretShares: Map<bigint, bigint>,
): bigint => {
	let signingShare = 0n;
	for (const [, share] of secretShares) {
		signingShare = mod_n(signingShare + share);
	}
	if (signingShare === 0n) throw Error("Could not calculate signing share!");
	return signingShare;
};

export const verifyKey = (publicKey: FrostPoint, privateKey: bigint): void => {
	const verification = g(privateKey);
	if (verification.x !== publicKey.x || verification.y !== publicKey.y) {
		throw Error("Private key and public key don't match!");
	}
};

export const evalPoly = (coefficient: bigint[], x: bigint): bigint => {
	if (x === 0n) {
		throw new Error("x is zero");
	}
	// Initialize the result to zero.
	let value = 0n;

	// Iterate over the coefficients in reverse order:
	const t = coefficient.length;
	for (let j = t - 1; j >= 0; j--) {
		// Multiply the current value by x (shift to the next power of x).
		value *= x;
		// Add the current coefficient to the value.
		value += coefficient[j];
		// Ensure the result is reduced to the field value.
		value = mod_n(value);
	}

	// Return the final evaluated value of the polynomial.
	return value;
};

export const evalCommitment = (
	commitments: readonly FrostPoint[],
	x: bigint,
): FrostPoint => {
	if (x === 0n) {
		throw new Error("x is zero");
	}
	let value = commitments[0];
	let term_pow = 1n;
	const t = commitments.length;
	for (let j = 1; j < t; j++) {
		term_pow = mod_n(term_pow * x);
		value = value.add(commitments[j].multiply(term_pow));
	}
	return value;
};
