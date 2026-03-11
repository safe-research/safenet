import { keccak256, stringToBytes } from "viem";
import { describe, expect, it } from "vitest";
import { addmod, g } from "../../frost/math.js";
import { groupChallenge, lagrangeCoefficient } from "./group.js";
import {
	bindingFactors,
	calculateGroupCommitment,
	generateNonce,
	groupCommitmentShare,
	type NonceCommitments,
} from "./nonces.js";
import { createSignatureShare, lagrangeChallenge } from "./shares.js";
import { verifySignature, verifySignatureShare } from "./verify.js";

// Deterministic private key and message for a single-signer scenario (threshold=1, participants=1)
const SK = 42n;
const PK = g(SK);
const MESSAGE = keccak256(stringToBytes("hello safenet"));

// Fixed randomness (32 bytes) for deterministic nonces
const RANDOMNESS_HIDING = new Uint8Array(32).fill(1);
const RANDOMNESS_BINDING = new Uint8Array(32).fill(2);

function buildScenario() {
	const hidingNonce = generateNonce(SK, RANDOMNESS_HIDING);
	const bindingNonce = generateNonce(SK, RANDOMNESS_BINDING);
	const nonces: NonceCommitments = {
		hidingNonce,
		bindingNonce,
		hidingNonceCommitment: g(hidingNonce),
		bindingNonceCommitment: g(bindingNonce),
	};

	const signers = [1n];
	const nonceMap = new Map<bigint, NonceCommitments>([[1n, nonces]]);

	// Compute binding factors
	const bfs = bindingFactors(PK, signers, nonceMap, MESSAGE);
	const bf = bfs[0].bindingFactor;

	// Compute group commitment
	const commitShare = groupCommitmentShare(bf, nonces);
	const groupCommitment = calculateGroupCommitment([commitShare]);

	// Compute challenge
	const challenge = groupChallenge(groupCommitment, PK, MESSAGE);

	// With a single signer, lagrange coefficient is trivially 1n
	const lagCoeff = lagrangeCoefficient(signers, 1n);
	const lagChallenge = lagrangeChallenge(lagCoeff, challenge);

	// Compute signature share
	const share = createSignatureShare(SK, nonces, bf, lagChallenge);

	return { nonces, bf, commitShare, groupCommitment, challenge, lagChallenge, share };
}

describe("verifySignature", () => {
	it("verifies a valid FROST signature for a single-signer scenario", () => {
		const { groupCommitment, share } = buildScenario();
		expect(verifySignature(groupCommitment, share, PK, MESSAGE)).toBe(true);
	});

	it("returns false for a tampered message", () => {
		const { groupCommitment, share } = buildScenario();
		const tamperedMessage = keccak256(stringToBytes("tampered message"));
		expect(verifySignature(groupCommitment, share, PK, tamperedMessage)).toBe(false);
	});

	it("returns false for a tampered group commitment", () => {
		const { share } = buildScenario();

		// Build a different group commitment using a different nonce
		const altHidingNonce = generateNonce(99n, new Uint8Array(32).fill(9));
		const altBindingNonce = generateNonce(99n, new Uint8Array(32).fill(8));
		const altNonces: NonceCommitments = {
			hidingNonce: altHidingNonce,
			bindingNonce: altBindingNonce,
			hidingNonceCommitment: g(altHidingNonce),
			bindingNonceCommitment: g(altBindingNonce),
		};
		const altCommitShare = groupCommitmentShare(3n, altNonces);
		const tamperedGroupCommitment = calculateGroupCommitment([altCommitShare]);

		expect(verifySignature(tamperedGroupCommitment, share, PK, MESSAGE)).toBe(false);
	});
});

describe("verifySignatureShare", () => {
	it("verifies a valid individual signature share", () => {
		const { commitShare, lagChallenge, share } = buildScenario();

		// verificationShare is g(SK) when there is no DKG (trivial single-participant case)
		const verificationShare = PK;

		// NOTE: verifySignatureShare uses x-coordinate-only comparison per FROST spec.
		// This tests the known behavior: only sG.x === (groupCommitmentShare + verificationShare * lagrangeChallenge).x
		expect(verifySignatureShare(share, verificationShare, lagChallenge, commitShare)).toBe(true);
	});

	it("returns false for a wrong signature share", () => {
		const { commitShare, lagChallenge, share } = buildScenario();
		const verificationShare = PK;

		// Add 1 to the valid share to make it invalid
		const wrongShare = addmod(share, 1n);
		expect(verifySignatureShare(wrongShare, verificationShare, lagChallenge, commitShare)).toBe(false);
	});
});
