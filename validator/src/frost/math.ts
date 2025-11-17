import { randomBytes } from "node:crypto";
import { mod } from "@noble/curves/abstract/modular.js";
import { secp256k1 } from "@noble/curves/secp256k1.js";
import { bytesToNumberBE } from "@noble/curves/utils.js";
import type { FrostPoint } from "./types.js";

export const G_BASE = secp256k1.Point.BASE;
export const N = secp256k1.Point.CURVE().n;

export const randomBigInt = () => bytesToNumberBE(randomBytes(32));

export const g = (scalar: bigint): FrostPoint => {
	const point = G_BASE.multiply(scalar);
	point.assertValidity();
	return point;
};

export const mod_n = (x: bigint) => {
	return mod(x, N);
};

export const neg = (val: bigint) => {
	return secp256k1.Point.Fn.neg(val);
};

export const addmod = (ĺhs: bigint, rhs: bigint) => {
	return secp256k1.Point.Fn.add(ĺhs, rhs);
};

export const submod = (ĺhs: bigint, rhs: bigint) => {
	return secp256k1.Point.Fn.sub(ĺhs, rhs);
};

export const mulmod = (ĺhs: bigint, rhs: bigint) => {
	return secp256k1.Point.Fn.mul(ĺhs, rhs);
};

export const divmod = (ĺhs: bigint, rhs: bigint) => {
	return secp256k1.Point.Fn.div(ĺhs, rhs);
};

export const toPoint = (coordinates: { x: bigint; y: bigint }): FrostPoint => {
	const point = secp256k1.Point.fromAffine(coordinates);
	point.assertValidity();
	return point;
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
		signingShare = addmod(signingShare, share);
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

export const evalPoly = (coefficient: readonly bigint[], x: bigint): bigint => {
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
	let value = commitments[0];
	if (x === 0n) {
		return value;
	}
	let term_pow = 1n;
	const t = commitments.length;
	for (let j = 1; j < t; j++) {
		term_pow = mulmod(term_pow, x);
		value = value.add(commitments[j].multiply(term_pow));
	}
	return value;
};
