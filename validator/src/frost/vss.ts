import { randomBytes } from "node:crypto";
import { hdkg, hpok, keyGenChallenge } from "./hashes.js";
import { addmod, g, mulmod } from "./math.js";
import type { FrostPoint, ProofOfKnowledge } from "./types.js";

/*
 * This is a modified or extended Pedersen DKG and
 * not just Feldmann VSS (the former is built on the latter).
 */

// Round 1.1
const generateCoefficient = (): bigint => {
	return hdkg(randomBytes(32));
};
export const createCoefficients = (threshold: number): bigint[] => {
	const coefficients: bigint[] = [];
	for (let i = 0; i < threshold; i++) {
		coefficients.push(generateCoefficient());
	}
	return coefficients;
};

// Round 1.2
const generateProofOfKnowledgeNonce = (): bigint => {
	return hpok(randomBytes(32));
};
export const createProofOfKnowledge = (id: bigint, coefficients: bigint[]): ProofOfKnowledge => {
	const a0 = coefficients[0];
	const ga0 = g(a0);
	const k = generateProofOfKnowledgeNonce();
	const r = g(k);
	const c = keyGenChallenge(id, ga0, r);
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
export const verifyCommitments = (id: bigint, commitments: readonly FrostPoint[], proof: ProofOfKnowledge): boolean => {
	const ga0 = commitments[0];
	const c = keyGenChallenge(id, ga0, proof.r);
	const v = g(proof.mu).add(ga0.multiply(c).negate());
	return proof.r.equals(v);
};
