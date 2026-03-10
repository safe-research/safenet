import { describe, expect, it } from "vitest";
import {
	addmod,
	createSigningShare,
	createVerificationShare,
	divmod,
	evalCommitment,
	evalPoly,
	G_BASE,
	g,
	mulmod,
	N,
	neg,
	pointFromBytes,
	scalarFromBytes,
	scalarToBytes,
	submod,
	toPoint,
	verifyKey,
} from "./math.js";

describe("math", () => {
	describe("g() — scalar multiplication", () => {
		it("g(1n) equals G_BASE", () => {
			expect(g(1n).equals(G_BASE)).toBe(true);
		});

		it("g(2n) equals G_BASE.double()", () => {
			expect(g(2n).equals(G_BASE.double())).toBe(true);
		});

		it("g(N - 1n) equals G_BASE.negate()", () => {
			expect(g(N - 1n).equals(G_BASE.negate())).toBe(true);
		});
	});

	describe("neg()", () => {
		it("addmod(a, neg(a)) === 0n", () => {
			const a = 42n;
			expect(addmod(a, neg(a))).toBe(0n);
		});
	});

	describe("addmod()", () => {
		it("is commutative: addmod(a, b) === addmod(b, a)", () => {
			const a = 12345678901234567890n;
			const b = 98765432109876543210n;
			expect(addmod(a, b)).toBe(addmod(b, a));
		});

		it("addmod(a, 0n) === a", () => {
			const a = 999999999999999999n;
			expect(addmod(a, 0n)).toBe(a);
		});
	});

	describe("submod()", () => {
		it("submod(a, a) === 0n", () => {
			const a = 12345678901234567890n;
			expect(submod(a, a)).toBe(0n);
		});

		it("submod(a, 0n) === a", () => {
			const a = 999999999999999999n;
			expect(submod(a, 0n)).toBe(a);
		});
	});

	describe("mulmod()", () => {
		it("mulmod(a, 1n) === a", () => {
			const a = 12345678901234567890n;
			expect(mulmod(a, 1n)).toBe(a);
		});

		it("mulmod(a, 0n) === 0n", () => {
			const a = 12345678901234567890n;
			expect(mulmod(a, 0n)).toBe(0n);
		});
	});

	describe("divmod()", () => {
		it("mulmod(divmod(a, b), b) === a (div is inverse of mul)", () => {
			const a = 12345678901234567890n;
			const b = 98765432109876543210n;
			expect(mulmod(divmod(a, b), b)).toBe(a);
		});
	});

	describe("toPoint() / pointFromBytes() — round-trip", () => {
		it("toPoint({x: G_BASE.x, y: G_BASE.y}) equals G_BASE", () => {
			const point = toPoint({ x: G_BASE.x, y: G_BASE.y });
			expect(point.equals(G_BASE)).toBe(true);
		});

		it("pointFromBytes(G_BASE.toBytes(true)) equals G_BASE (compressed round-trip)", () => {
			const bytes = G_BASE.toBytes(true);
			const point = pointFromBytes(bytes);
			expect(point.equals(G_BASE)).toBe(true);
		});

		it("toPoint with invalid coordinates throws", () => {
			expect(() => toPoint({ x: 0n, y: 0n })).toThrow();
		});
	});

	describe("scalarToBytes() / scalarFromBytes() — round-trip", () => {
		it("scalarFromBytes(scalarToBytes(42n)) === 42n", () => {
			expect(scalarFromBytes(scalarToBytes(42n))).toBe(42n);
		});

		it("round-trip for boundary value 1n", () => {
			expect(scalarFromBytes(scalarToBytes(1n))).toBe(1n);
		});

		it("round-trip for boundary value N - 1n", () => {
			expect(scalarFromBytes(scalarToBytes(N - 1n))).toBe(N - 1n);
		});
	});

	describe("evalPoly()", () => {
		it("single coefficient [c]: evalPoly([c], 1n) === c", () => {
			const c = 7n;
			expect(evalPoly([c], 1n)).toBe(c);
		});

		it("two coefficients [a0, a1]: evalPoly([a0, a1], x) === addmod(a0, mulmod(a1, x))", () => {
			const a0 = 3n;
			const a1 = 5n;
			const x = 4n;
			expect(evalPoly([a0, a1], x)).toBe(addmod(a0, mulmod(a1, x)));
		});

		it("throws on x === 0n", () => {
			expect(() => evalPoly([1n, 2n], 0n)).toThrow("x is zero");
		});
	});

	describe("evalCommitment()", () => {
		it("for x != 0n: evalCommitment([g(a0), g(a1)], x) equals g(evalPoly([a0, a1], x))", () => {
			const a0 = 3n;
			const a1 = 5n;
			const x = 4n;
			const commitments = [g(a0), g(a1)];
			const expected = g(evalPoly([a0, a1], x));
			expect(evalCommitment(commitments, x).equals(expected)).toBe(true);
		});

		it("for x === 0n: returns commitments[0] without throwing (diverges from evalPoly)", () => {
			const a0 = 3n;
			const a1 = 5n;
			const commitments = [g(a0), g(a1)];
			// evalPoly would throw, but evalCommitment returns commitments[0]
			expect(() => evalCommitment(commitments, 0n)).not.toThrow();
			expect(evalCommitment(commitments, 0n).equals(commitments[0])).toBe(true);
		});
	});

	describe("createVerificationShare()", () => {
		it("with 2 participants, each with commitments: result equals expected point", () => {
			// Participant 1 has coefficients [a0_1, a1_1], participant 2 has [a0_2, a1_2]
			const a0_1 = 3n;
			const a1_1 = 5n;
			const a0_2 = 7n;
			const a1_2 = 11n;
			const senderId = 2n;

			const commitments1 = [g(a0_1), g(a1_1)];
			const commitments2 = [g(a0_2), g(a1_2)];

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

	describe("verifyKey()", () => {
		it("g(privKey) matches pubKey: returns true", () => {
			const privKey = 12345n;
			const pubKey = g(privKey);
			expect(verifyKey(pubKey, privKey)).toBe(true);
		});

		it("wrong private key: returns false", () => {
			const privKey = 12345n;
			const pubKey = g(privKey);
			expect(verifyKey(pubKey, privKey + 1n)).toBe(false);
		});
	});
});
