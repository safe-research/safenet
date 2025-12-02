import { hexToBytes } from "viem";
import { describe, expect, it } from "vitest";
import { h1, h2, h3, h4, h5, keyGenChallenge } from "./hashes.js";
import { toPoint } from "./math.js";

// --- Tests ---
describe("hashes", () => {
	it("should generate correct h1 hash", () => {
		const hash = h1(hexToBytes("0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b"));
		expect(hash).toBe(65366193696860196695414947064821663180241281339071613554598903411371557073280n);
	});
	it("should generate correct h2 hash", () => {
		const hash = h2(hexToBytes("0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b"));
		expect(hash).toBe(33150593925562805502779376598105657283445871999808781975649610745815960364725n);
	});
	it("should generate correct h3 hash", () => {
		const hash = h3(hexToBytes("0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b"));
		expect(hash).toBe(57947748375997171466674059397073681554613456900543402678765823935581620592241n);
	});
	it("should generate correct h4 hash", () => {
		const hash = h4(hexToBytes("0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b"));
		expect(hash).toStrictEqual(hexToBytes("0x9b0cbeb8a7132d10a14f2b80fcce19e73b96ce7ce30e8c33615aab7767804777"));
	});
	it("should generate correct h5 hash", () => {
		const hash = h5(hexToBytes("0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b"));
		expect(hash).toStrictEqual(hexToBytes("0xe7e18b7374674a612aa7fbfba2741c09e9959ccb5b65fd2add9688b9e51d9f25"));
	});
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
