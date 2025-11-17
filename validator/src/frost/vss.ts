import { keyGenChallenge } from "./hashes.js";
import { addmod, g, mod_n, mulmod, randomBigInt } from "./math.js";
import type { FrostPoint, ProofOfKnowledge } from "./types.js";

/*
 * This is a modified or extended Pedersen DKG and
 * not just Feldmann VSS (the former is built on the latter).
 */

// Round 1.1
export const createCoefficients = (threshold: bigint): bigint[] => {
	const coefficients: bigint[] = [];
	for (let i = 0; i < threshold; i++) {
		coefficients.push(mod_n(randomBigInt()));
	}
	return coefficients;
};

// Round 1.2
export const createProofOfKnowledge = (
	index: bigint,
	coefficients: bigint[],
): ProofOfKnowledge => {
	const a0 = coefficients[0];
	const ga0 = g(a0);
	const k = randomBigInt();
	const r = g(k);
	const c = keyGenChallenge(index, ga0, r);
	const mu = addmod(k, mulmod(a0, c));
	return {
		r,
		mu,
	};
};

// Round 1.3
export const createCommitments = (coefficients: bigint[]): FrostPoint[] => {
	return coefficients.map((a) => g(a));
};

// Round 1.5
// Note: this only verifies commitment[0], other commitments are implicitly verified in round 2
export const verifyCommitments = (
	index: bigint,
	commitments: readonly FrostPoint[],
	proof: ProofOfKnowledge,
) => {
	const ga0 = commitments[0];
	const c = keyGenChallenge(index, ga0, proof.r);
	const v = g(proof.mu).add(ga0.multiply(c).negate());
	if (!proof.r.equals(v)) throw Error(`Invalid commitments for ${index}`);
};
