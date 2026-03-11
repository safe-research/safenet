import { keccak256, stringToBytes } from "viem";
import { describe, expect, it } from "vitest";
import { divmod, N, submod, toPoint } from "../../frost/math.js";
import { groupChallenge, lagrangeCoefficient } from "./group.js";

describe("groupChallenge", () => {
	const groupCommitment = toPoint({
		x: 0x8a3802114b5b6369ae8ba7822bdb029dee0d53fc416225d9198959b83f73215bn,
		y: 0x3020f80cae8f515d58686d5c6e4f1d027a1671348b6402f4e43ce525bda00fbcn,
	});
	const groupPublicKey = toPoint({
		x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75n,
		y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5n,
	});
	const message = keccak256(stringToBytes("hello"));

	it("should generate correct challenge", () => {
		expect(groupChallenge(groupCommitment, groupPublicKey, message)).toBe(
			0x092370ad82e7356eb5fe89e9be058a335705b482eaa9832fb81eddd3723647b4n,
		);
	});
});

describe("lagrangeCoefficient", () => {
	it("signers=[1n,2n], id=1n: numerator=2n, denominator=submod(2n,1n)=1n → result=2n", () => {
		const result = lagrangeCoefficient([1n, 2n], 1n);
		expect(result).toBe(2n);
	});

	it("signers=[1n,2n], id=2n: numerator=1n, denominator=submod(1n,2n)=N-1n → result=neg(1n)=N-1n", () => {
		const result = lagrangeCoefficient([1n, 2n], 2n);
		expect(result).toBe(N - 1n);
	});

	it("signers=[1n,3n], id=1n: numerator=3n, denominator=submod(3n,1n)=2n → result=divmod(3n,2n)", () => {
		const expected = divmod(3n, submod(3n, 1n));
		const result = lagrangeCoefficient([1n, 3n], 1n);
		expect(result).toBe(expected);
	});

	it("throws when id is not in signers", () => {
		expect(() => lagrangeCoefficient([1n, 2n], 3n)).toThrow("3 not part of signers");
	});

	it("single signer returns 1n (trivial lagrange coefficient)", () => {
		const result = lagrangeCoefficient([1n], 1n);
		expect(result).toBe(1n);
	});
});
