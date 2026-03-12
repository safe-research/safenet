import { describe, expect, it } from "vitest";
import { g } from "./math.js";
import { createCoefficients, createCommitments, createProofOfKnowledge, verifyCommitments } from "./vss.js";

describe("vss", () => {
	describe("verifyCommitments(id, commitments, proof)", () => {
		it("valid proof returns true", () => {
			const id = 42n;
			const coefficients = createCoefficients(3);
			const commitments = createCommitments(coefficients);
			const proof = createProofOfKnowledge(id, coefficients);
			expect(verifyCommitments(id, commitments, proof)).toBe(true);
		});

		it("tampered proof (flip r to a different point) returns false", () => {
			const id = 42n;
			const coefficients = createCoefficients(2);
			const commitments = createCommitments(coefficients);
			const proof = createProofOfKnowledge(id, coefficients);

			// Use a different point for r (G_BASE itself, which is unlikely to equal proof.r)
			const tamperedProof = { ...proof, r: g(999n) };

			expect(verifyCommitments(id, commitments, tamperedProof)).toBe(false);
		});

		it("wrong id returns false (different challenge hash)", () => {
			const id = 42n;
			const wrongId = 99n;
			const coefficients = createCoefficients(2);
			const commitments = createCommitments(coefficients);
			const proof = createProofOfKnowledge(id, coefficients);

			expect(verifyCommitments(wrongId, commitments, proof)).toBe(false);
		});
	});
});
