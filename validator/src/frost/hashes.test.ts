import { hexToBytes } from "viem";
import { describe, expect, it } from "vitest"; // or '@jest/globals'
import { h1, h2, h3, h4, h5 } from "./hashes.js";

// --- Tests ---
describe("hashes", () => {
	it("should generate correct h1 hash", () => {
		const hash = h1(
			hexToBytes(
				"0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b",
			),
		);
		expect(hash).toBe(
			65366193696860196695414947064821663180241281339071613554598903411371557073280n,
		);
	});
	it("should generate correct h2 hash", () => {
		const hash = h2(
			hexToBytes(
				"0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b",
			),
		);
		expect(hash).toBe(
			33150593925562805502779376598105657283445871999808781975649610745815960364725n,
		);
	});
	it("should generate correct h3 hash", () => {
		const hash = h3(
			hexToBytes(
				"0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b",
			),
		);
		expect(hash).toBe(
			57947748375997171466674059397073681554613456900543402678765823935581620592241n,
		);
	});
	it("should generate correct h4 hash", () => {
		const hash = h4(
			hexToBytes(
				"0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b",
			),
		);
		expect(hash).toBe(
			"0x9b0cbeb8a7132d10a14f2b80fcce19e73b96ce7ce30e8c33615aab7767804777",
		);
	});
	it("should generate correct h5 hash", () => {
		const hash = h5(
			hexToBytes(
				"0x37e58bc84afff4e1afade4140135583af3d6d3523a435e60cec5dc75ae3d7e8b",
			),
		);
		expect(hash).toBe(
			"0xe7e18b7374674a612aa7fbfba2741c09e9959ccb5b65fd2add9688b9e51d9f25",
		);
	});
});
