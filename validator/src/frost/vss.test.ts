import { describe, expect, it } from "vitest";
import { g } from "./math.js";
import {
	createCoefficients,
	createCommitments,
	createEncryptionKey,
	createProofOfKnowledge,
	verifyCommitments,
} from "./vss.js";

describe("vss", () => {
	describe("createEncryptionKey()", () => {
		it("publicKey equals g(secretKey)", () => {
			const { secretKey, publicKey } = createEncryptionKey();
			expect(publicKey.equals(g(secretKey))).toBe(true);
		});

		it("two calls produce different keys", () => {
			const key1 = createEncryptionKey();
			const key2 = createEncryptionKey();
			// It's probabilistically certain they differ
			expect(key1.secretKey === key2.secretKey).toBe(false);
		});
	});

	describe("createCoefficients(threshold)", () => {
		it("returns array of length 1 for threshold 1", () => {
			const coeffs = createCoefficients(1);
			expect(coeffs).toHaveLength(1);
		});

		it("returns array of length 2 for threshold 2", () => {
			const coeffs = createCoefficients(2);
			expect(coeffs).toHaveLength(2);
		});

		it("returns array of length 3 for threshold 3", () => {
			const coeffs = createCoefficients(3);
			expect(coeffs).toHaveLength(3);
		});

		it("all elements are bigints", () => {
			const coeffs = createCoefficients(3);
			for (const c of coeffs) {
				expect(typeof c).toBe("bigint");
			}
		});
	});

	describe("createCommitments(coefficients)", () => {
		it("commitments[i] equals g(coefficients[i]) for each i", () => {
			const coefficients = createCoefficients(3);
			const commitments = createCommitments(coefficients);
			expect(commitments).toHaveLength(coefficients.length);
			for (let i = 0; i < coefficients.length; i++) {
				expect(commitments[i].equals(g(coefficients[i]))).toBe(true);
			}
		});
	});

	describe("createProofOfKnowledge(id, coefficients)", () => {
		it("returned proof has r (a point) and mu (a bigint)", () => {
			const id = 1n;
			const coefficients = createCoefficients(2);
			const proof = createProofOfKnowledge(id, coefficients);
			expect(typeof proof.mu).toBe("bigint");
			// r is a point — check it has x/y coordinates
			expect(typeof proof.r.x).toBe("bigint");
			expect(typeof proof.r.y).toBe("bigint");
		});

		it("returned proof verifies via verifyCommitments", () => {
			const id = 1n;
			const coefficients = createCoefficients(2);
			const commitments = createCommitments(coefficients);
			const proof = createProofOfKnowledge(id, coefficients);
			expect(verifyCommitments(id, commitments, proof)).toBe(true);
		});
	});

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
			const { publicKey: differentPoint } = createEncryptionKey();
			const tamperedProof = { ...proof, r: differentPoint };

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
