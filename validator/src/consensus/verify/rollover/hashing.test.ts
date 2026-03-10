import { describe, expect, it } from "vitest";
import { epochRolloverHash } from "./hashing.js";

const TEST_ADDRESS = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" as const;
const OTHER_ADDRESS = "0x71C7656EC7ab88b098defB751B7401B5f6d8976F" as const;

const testPacket = {
	type: "epoch_rollover_packet" as const,
	domain: {
		chain: 1n,
		consensus: TEST_ADDRESS,
	},
	rollover: {
		activeEpoch: 1n,
		proposedEpoch: 2n,
		rolloverBlock: 100n,
		groupKeyX: 12345n,
		groupKeyY: 67890n,
	},
};

describe("epochRolloverHash", () => {
	it("deterministic: same packet → same hash", () => {
		const hash1 = epochRolloverHash(testPacket);
		const hash2 = epochRolloverHash({ ...testPacket });
		expect(hash1).toBe(hash2);
	});

	it("different activeEpoch → different hash", () => {
		const hash1 = epochRolloverHash(testPacket);
		const hash2 = epochRolloverHash({
			...testPacket,
			rollover: { ...testPacket.rollover, activeEpoch: 99n },
		});
		expect(hash1).not.toBe(hash2);
	});

	it("different proposedEpoch → different hash", () => {
		const hash1 = epochRolloverHash(testPacket);
		const hash2 = epochRolloverHash({
			...testPacket,
			rollover: { ...testPacket.rollover, proposedEpoch: 99n },
		});
		expect(hash1).not.toBe(hash2);
	});

	it("different chain → different hash", () => {
		const hash1 = epochRolloverHash(testPacket);
		const hash2 = epochRolloverHash({
			...testPacket,
			domain: { ...testPacket.domain, chain: 137n },
		});
		expect(hash1).not.toBe(hash2);
	});

	it("different consensus address → different hash", () => {
		const hash1 = epochRolloverHash(testPacket);
		const hash2 = epochRolloverHash({
			...testPacket,
			domain: { ...testPacket.domain, consensus: OTHER_ADDRESS },
		});
		expect(hash1).not.toBe(hash2);
	});

	it("returns a 0x-prefixed hex string of length 66", () => {
		const hash = epochRolloverHash(testPacket);
		expect(hash).toMatch(/^0x[0-9a-f]{64}$/i);
		expect(hash.length).toBe(66);
	});
});
