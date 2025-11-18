import { describe, expect, it } from "vitest";
import { EpochRolloverHandler } from "./handler.js";
import type { EpochRolloverPacket } from "./schemas.js";

describe("epoch rollover handler", () => {
	it("should throw on invalid packet", async () => {
		const handler = new EpochRolloverHandler();
		await expect(
			handler.hashAndVerify({
				type: "invalid packet",
			} as unknown as EpochRolloverPacket),
		).rejects.toThrow();
	});

	it("should return correct hash", async () => {
		const handler = new EpochRolloverHandler();
		await expect(
			handler.hashAndVerify({
				type: "epoch_rollover_packet",
				domain: {
					chain: 23n,
					consensus: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
				},
				rollover: {
					activeEpoch: 0n,
					proposedEpoch: 1n,
					rolloverAt: 0xbaddad42n,
					groupKeyX:
						0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75n,
					groupKeyY:
						0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5n,
				},
			}),
		).resolves.toBe(
			"0x31e313d5239d0a1ffe5ab3bd4d9853d63a2fc30e2adf791e56834fbe68bc3f5f",
		);
	});
});
