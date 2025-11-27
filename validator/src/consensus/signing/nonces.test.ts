import { bytesToHex, hexToBytes, keccak256, stringToBytes } from "viem";
import { describe, expect, it } from "vitest";
import { g, toPoint } from "../../frost/math.js";
import {
	bindingFactors,
	bindingPrefix,
	generateNonce,
	type NonceCommitments,
} from "./nonces.js";

describe("nonces", () => {
	it("should generate correct nonce", async () => {
		const random = hexToBytes(
			"0x2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a",
		);
		const secret =
			0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80n;
		expect(generateNonce(secret, random)).toBe(
			0x03d979abaa17ca44e015f9e248c6cefc167ad21e814256f2a0a02cce70d57ba1n,
		);
	});

	it("should correctly encode binding prefix", async () => {
		const groupPublicKey = toPoint({
			x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75n,
			y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5n,
		});
		const signers = [1n, 2n, 3n];
		const commitments = new Map<bigint, NonceCommitments>();
		signers.forEach((p) => {
			commitments.set(p, {
				hidingNonce: 0xd0n + p,
				bindingNonce: 0xe0n + p,
				hidingNonceCommitment: g(0xd0n + p),
				bindingNonceCommitment: g(0xe0n + p),
			});
		});
		const message = keccak256(stringToBytes("hello"));
		expect(
			bytesToHex(bindingPrefix(groupPublicKey, signers, commitments, message)),
		).toBe(
			"0x038318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed753e3ff5d5672762f4c3add84cc8e383dc781e5f8f8f230913e114bae324ffbe64fa82f351f10ae44fb79bba17ffd42aba4370ec76c6e48328409c1a981ca3b50a",
		);
	});
	it("should generate correct binding factors", async () => {
		const groupPublicKey = toPoint({
			x: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75n,
			y: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5n,
		});
		const signers = [1n, 2n, 3n];
		const commitments = new Map<bigint, NonceCommitments>();
		signers.forEach((p) => {
			commitments.set(p, {
				hidingNonce: 0xd0n + p,
				bindingNonce: 0xe0n + p,
				hidingNonceCommitment: g(0xd0n + p),
				bindingNonceCommitment: g(0xe0n + p),
			});
		});
		const message = keccak256(stringToBytes("hello"));

		const factors = bindingFactors(
			groupPublicKey,
			signers,
			commitments,
			message,
		);
		expect(factors.length).toBe(3);
		expect(factors[0].bindingFactor).toBe(
			0x3ace394f1783cd2f9647aaded69596328f98cc57c823ae5652d7275461be9bean,
		);
		expect(factors[1].bindingFactor).toBe(
			0x30df3963e4aee100fa049ec729adf4e75609b4f3f699fa17cf1c593ef1cf3ecfn,
		);
		expect(factors[2].bindingFactor).toBe(
			0x04849a66886b4b59b920d847e334fc3f9aa355d8c152e146d3ed03c8c3a8096dn,
		);
	});
});
