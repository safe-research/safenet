import { describe, expect, it, vi } from "vitest";
import { EpochRolloverHandler } from "./handler.js";
import type { EpochRolloverPacket } from "./schemas.js";

const validPacket: EpochRolloverPacket = {
	type: "epoch_rollover_packet",
	domain: {
		chain: 23n,
		consensus: "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97",
	},
	rollover: {
		activeEpoch: 0n,
		proposedEpoch: 1n,
		rolloverBlock: 0xbaddad42n,
		groupKeyX: 0x8318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed75n,
		groupKeyY: 0x3547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5n,
	},
};

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
		await expect(handler.hashAndVerify(validPacket)).resolves.toBe(
			"0xc1e4d484d6c376741c904290cc043f4afb4618f9d567dcdd0edcbf22abae57f7",
		);
	});

	it("should throw when proposedEpoch is not activeEpoch + 1", async () => {
		const handler = new EpochRolloverHandler();
		await expect(
			handler.hashAndVerify({
				...validPacket,
				rollover: {
					...validPacket.rollover,
					activeEpoch: 5n,
					proposedEpoch: 7n,
				},
			}),
		).rejects.toThrow("proposedEpoch (7) must be activeEpoch (5) + 1");
	});

	it("should throw when proposedEpoch equals activeEpoch", async () => {
		const handler = new EpochRolloverHandler();
		await expect(
			handler.hashAndVerify({
				...validPacket,
				rollover: {
					...validPacket.rollover,
					activeEpoch: 3n,
					proposedEpoch: 3n,
				},
			}),
		).rejects.toThrow("proposedEpoch (3) must be activeEpoch (3) + 1");
	});

	it("should call the check function when provided", async () => {
		const check = vi.fn();
		const handler = new EpochRolloverHandler(check);
		await handler.hashAndVerify(validPacket);
		expect(check).toBeCalledTimes(1);
		expect(check).toBeCalledWith(validPacket.rollover);
	});

	it("should throw when check function throws", async () => {
		const check = vi.fn().mockImplementation(() => {
			throw new Error("Epoch mismatch");
		});
		const handler = new EpochRolloverHandler(check);
		await expect(handler.hashAndVerify(validPacket)).rejects.toThrow("Epoch mismatch");
	});
});
