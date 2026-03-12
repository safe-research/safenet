import { describe, expect, it } from "vitest";
import { g } from "./math.js";
import { ecdh } from "./secret.js";

describe("ecdh", () => {
	// Use deterministic key pairs with small private keys
	const alicePriv = 2n;
	const alicePub = g(alicePriv);
	const bobPriv = 3n;
	const bobPub = g(bobPriv);
	const carolPriv = 5n;
	const carolPub = g(carolPriv);

	const msg = 0xdeadbeefcafebaben;

	it("round-trip: decrypt(encrypt(msg, alicePriv, bobPub), bobPriv, alicePub) === msg", () => {
		const encrypted = ecdh(msg, alicePriv, bobPub);
		const decrypted = ecdh(encrypted, bobPriv, alicePub);
		expect(decrypted).toBe(msg);
	});

	it("shared secret commutativity: ecdh(msg, alicePriv, bobPub) === ecdh(msg, bobPriv, alicePub)", () => {
		expect(ecdh(msg, alicePriv, bobPub)).toBe(ecdh(msg, bobPriv, alicePub));
	});

	it("different recipient produces different ciphertext", () => {
		expect(ecdh(msg, alicePriv, bobPub)).not.toBe(ecdh(msg, alicePriv, carolPub));
	});

	it("zero private key throws (contracts enforce non-zero via Secp256k1.requireNonZero)", () => {
		expect(() => ecdh(msg, 0n, bobPub)).toThrow();
	});
});
