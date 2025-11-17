import {
	type H2COpts,
	hash_to_field,
} from "@noble/curves/abstract/hash-to-curve.js";
import { secp256k1 } from "@noble/curves/secp256k1.js";
import { concatBytes, numberToBytesBE } from "@noble/curves/utils.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { stringToBytes } from "viem";
import type { FrostPoint } from "./types.js";

const N = secp256k1.Point.CURVE().n;
const CONTEXT = "FROST-secp256k1-SHA256-v1";

const dst = (discriminant: string): string => CONTEXT + discriminant;

const opts = (discriminant: string): H2COpts => {
	return {
		m: 1,
		p: N,
		k: 128,
		expand: "xmd",
		hash: sha256,
		DST: dst(discriminant),
	};
};

// TODO: replace by proper hashing function
export const keyGenChallenge = (
	index: bigint,
	ga0: FrostPoint,
	r: FrostPoint,
): bigint => {
	return hdkg(
		concatBytes(numberToBytesBE(index, 32), ga0.toBytes(true), r.toBytes(true)),
	);
};

export const hdkg = (input: Uint8Array): bigint => {
	return hash_to_field(input, 1, opts("dkg"))[0][0];
};

export const h1 = (input: Uint8Array): bigint => {
	return hash_to_field(input, 1, opts("rho"))[0][0];
};

export const h2 = (input: Uint8Array): bigint => {
	return hash_to_field(input, 1, opts("chal"))[0][0];
};

export const h3 = (input: Uint8Array): bigint => {
	return hash_to_field(input, 1, opts("nonce"))[0][0];
};

export const h4 = (input: Uint8Array): Uint8Array => {
	const dstBytes = stringToBytes(dst("msg"));
	return sha256(concatBytes(dstBytes, input));
};

export const h5 = (input: Uint8Array): Uint8Array => {
	const dstBytes = stringToBytes(dst("com"));
	return sha256(concatBytes(dstBytes, input));
};
