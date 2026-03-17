import { describe, expect, it } from "vitest";
import { g, toPoint } from "./math.js";
import {
	createCoefficients,
	createCommitments,
	createProofOfKnowledge,
	keyGenChallenge,
	verifyCommitments,
} from "./vss.js";

describe("vss", () => {
	describe("keyGenChallenge", () => {
		it("should generate a valid key gen challenge", () => {
			const ga0 = toPoint({
				x: BigInt("0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75"),
				y: BigInt("0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5"),
			});
			const r = toPoint({
				x: BigInt("0x8a3802114b5b6369ae8ba7822bdb029dee0d53fc416225d9198959b83f73215b"),
				y: BigInt("0x3020f80cae8f515d58686d5c6e4f1d027a1671348b6402f4e43ce525bda00fbc"),
			});
			const hash = keyGenChallenge(1n, ga0, r);
			expect(hash).toBe(BigInt("0xe39fcb3eef980ce5ee77898a6ed247fe78146aca2852ca4cf9f7fdcf23b4d470"));
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
