import { describe, expect, it } from "vitest";
import { addmod, createSigningShare, createVerificationShare, evalCommitment, g } from "./math.js";

describe("math", () => {
	describe("createVerificationShare()", () => {
		it("with 2 participants, each with commitments: result equals expected point", () => {
			// Participant 1 has coefficients [a0_1, a1_1], participant 2 has [a0_2, a1_2]
			const coefficients = {
				p1: [3n, 5n],
				p2: [7n, 11n],
			};
			const senderId = 2n;

			const commitments1 = coefficients.p1.map(g);
			const commitments2 = coefficients.p2.map(g);

			const allCommitments = new Map<bigint, readonly ReturnType<typeof g>[]>([
				[1n, commitments1],
				[2n, commitments2],
			]);

			const result = createVerificationShare(allCommitments, senderId);

			// The expected result is evalCommitment(commitments1, senderId) + evalCommitment(commitments2, senderId)
			const expected = evalCommitment(commitments1, senderId).add(evalCommitment(commitments2, senderId));
			expect(result.equals(expected)).toBe(true);
		});

		it("empty map throws", () => {
			const emptyMap = new Map<bigint, readonly ReturnType<typeof g>[]>();
			expect(() => createVerificationShare(emptyMap, 1n)).toThrow("Could not calculate verification share!");
		});
	});

	describe("createSigningShare()", () => {
		it("returns addmod of all shares in map", () => {
			const share1 = 3n;
			const share2 = 5n;
			const secretShares = new Map<bigint, bigint>([
				[1n, share1],
				[2n, share2],
			]);
			expect(createSigningShare(secretShares)).toBe(addmod(share1, share2));
		});

		it("empty map throws because signingShare stays at 0n", () => {
			const emptyMap = new Map<bigint, bigint>();
			expect(() => createSigningShare(emptyMap)).toThrow("Could not calculate signing share!");
		});
	});
});
