import { type H2COpts, hash_to_field } from "@noble/curves/abstract/hash-to-curve.js";
import { secp256k1 } from "@noble/curves/secp256k1.js";
import { sha256 } from "@noble/hashes/sha2.js";

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

export const h2 = (input: Uint8Array): bigint => {
	return hash_to_field(input, 1, opts("chal"))[0][0];
};
