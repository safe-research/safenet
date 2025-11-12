import { g, hashToBigInt, mod_n, randomBigInt } from "./math.js";
import type { FrostPoint, GroupId, ProofOfKnowledge } from "./types.js";

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
	groupId: GroupId,
	index: bigint,
	coefficients: bigint[],
): ProofOfKnowledge => {
	const a0 = coefficients[0];
	const ga0 = g(a0);
	const k = randomBigInt();
	const r = g(k);
	const c = hashToBigInt(index, ga0, r, groupId);
	const mu = mod_n(k + a0 * c);
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
	groupId: GroupId,
	index: bigint,
	commitments: readonly FrostPoint[],
	proof: ProofOfKnowledge,
) => {
	const ga0 = commitments[0];
	const c = hashToBigInt(index, ga0, proof.r, groupId);
	const v = g(proof.mu).add(ga0.multiply(c).negate());
	if (!proof.r.equals(v))
		throw Error(`Invalid commitments for ${groupId}:${index}`);
};
