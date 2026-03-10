import { describe, expect, it } from "vitest";
import { addmod, mulmod } from "../../frost/math.js";
import { createSignatureShare, lagrangeChallenge } from "./shares.js";

describe("lagrangeChallenge", () => {
	it("lagrangeChallenge(1n, c) === c", () => {
		const c = 12345678901234567890n;
		expect(lagrangeChallenge(1n, c)).toBe(c);
	});

	it("lagrangeChallenge(0n, c) === 0n", () => {
		const c = 9876543210n;
		expect(lagrangeChallenge(0n, c)).toBe(0n);
	});

	it("computes mulmod(lagCoeff, challenge) with known values", () => {
		const lagCoeff = 2n;
		const challenge = 3n;
		const result = lagrangeChallenge(lagCoeff, challenge);
		expect(result).toBe(mulmod(challenge, lagCoeff));
	});

	it("is deterministic for same inputs", () => {
		const lagCoeff = 7n;
		const challenge = 99999999999999999999n;
		const r1 = lagrangeChallenge(lagCoeff, challenge);
		const r2 = lagrangeChallenge(lagCoeff, challenge);
		expect(r1).toBe(r2);
	});
});

describe("createSignatureShare", () => {
	it("is algebraically correct: share = hidingNonce + bindingNonce*bindingFactor + lagrangeChallenge*privateKey", () => {
		const hidingNonce = 1000n;
		const bindingNonce = 2000n;
		const bindingFactor = 3n;
		const lagChallenge = 5n;
		const privateKey = 42n;

		const nonces = { hidingNonce, bindingNonce };
		const result = createSignatureShare(privateKey, nonces, bindingFactor, lagChallenge);

		// Manual calculation using same modular arithmetic
		const expected = addmod(hidingNonce, addmod(mulmod(bindingNonce, bindingFactor), mulmod(lagChallenge, privateKey)));
		expect(result).toBe(expected);
	});

	it("is deterministic for same inputs", () => {
		const nonces = { hidingNonce: 111n, bindingNonce: 222n };
		const bindingFactor = 333n;
		const lagChallenge = 444n;
		const privateKey = 555n;

		const r1 = createSignatureShare(privateKey, nonces, bindingFactor, lagChallenge);
		const r2 = createSignatureShare(privateKey, nonces, bindingFactor, lagChallenge);
		expect(r1).toBe(r2);
	});

	it("different private keys produce different shares", () => {
		const nonces = { hidingNonce: 100n, bindingNonce: 200n };
		const bindingFactor = 3n;
		const lagChallenge = 5n;

		const share1 = createSignatureShare(42n, nonces, bindingFactor, lagChallenge);
		const share2 = createSignatureShare(137n, nonces, bindingFactor, lagChallenge);
		expect(share1).not.toBe(share2);
	});

	it("different binding factors produce different shares", () => {
		const nonces = { hidingNonce: 100n, bindingNonce: 200n };
		const lagChallenge = 5n;
		const privateKey = 42n;

		const share1 = createSignatureShare(privateKey, nonces, 3n, lagChallenge);
		const share2 = createSignatureShare(privateKey, nonces, 4n, lagChallenge);
		expect(share1).not.toBe(share2);
	});
});
