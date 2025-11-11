import {
	type H2COpts,
	hash_to_field,
} from "@noble/curves/abstract/hash-to-curve.js";
import { secp256k1 } from "@noble/curves/secp256k1.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { bytesToHex, concatBytes, type Hex, stringToBytes } from "viem";

const N = secp256k1.Point.CURVE().n;
const DOMAIN = "FROST-secp256k1-SHA256-v1";

const dst = (context: string): string => DOMAIN + context;

const opts = (context: string): H2COpts => {
	return { m: 1, p: N, k: 128, expand: "xmd", hash: sha256, DST: dst(context) };
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

export const h4 = (input: Uint8Array): Hex => {
	const dstBytes = stringToBytes(dst("msg"));
	return bytesToHex(sha256(concatBytes([dstBytes, input])));
};

export const h5 = (input: Uint8Array): Hex => {
	const dstBytes = stringToBytes(dst("com"));
	return bytesToHex(sha256(concatBytes([dstBytes, input])));
};
