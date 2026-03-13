import { ethAddress } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it } from "vitest";
import type { ParticipantInfo } from "../types/interfaces.js";
import { participantsForEpoch } from "./participants.js";

const PARTICIPANTS_INFO: ParticipantInfo[] = [
	{
		address: entryPoint06Address,
		activeFrom: 0n,
	},
	{
		address: entryPoint07Address,
		activeFrom: 0n,
	},
	{
		address: entryPoint08Address,
		activeFrom: 1n,
		activeBefore: 3n,
	},
	{
		address: ethAddress,
		activeFrom: 2n,
	},
];

describe("participants", () => {
	it("should correctly evaluate ranges", async () => {
		expect(participantsForEpoch(PARTICIPANTS_INFO, 0n)).toStrictEqual([entryPoint06Address, entryPoint07Address]);
		expect(participantsForEpoch(PARTICIPANTS_INFO, 1n)).toStrictEqual([
			entryPoint06Address,
			entryPoint07Address,
			entryPoint08Address,
		]);
		expect(participantsForEpoch(PARTICIPANTS_INFO, 2n)).toStrictEqual([
			entryPoint06Address,
			entryPoint07Address,
			entryPoint08Address,
			ethAddress,
		]);
		expect(participantsForEpoch(PARTICIPANTS_INFO, 3n)).toStrictEqual([
			entryPoint06Address,
			entryPoint07Address,
			ethAddress,
		]);
	});

	it("should preserve input order", async () => {
		expect(participantsForEpoch(PARTICIPANTS_INFO.toReversed(), 2n)).toStrictEqual([
			ethAddress,
			entryPoint08Address,
			entryPoint07Address,
			entryPoint06Address,
		]);
	});

	it("should remove duplicates", async () => {
		expect(participantsForEpoch([...PARTICIPANTS_INFO, ...PARTICIPANTS_INFO], 0n)).toStrictEqual(
			participantsForEpoch(PARTICIPANTS_INFO, 0n),
		);
	});
});
