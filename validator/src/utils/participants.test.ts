import { ethAddress } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it } from "vitest";
import type { Participant } from "../consensus/storage/types.js";
import type { ParticipantInfo } from "../types/interfaces.js";
import { participantsForEpoch } from "./participants.js";

const PARTICIPANTS_INFO: ParticipantInfo[] = [
	{
		id: 1n,
		address: entryPoint06Address,
		activeFrom: 0n,
	},
	{
		id: 2n,
		address: entryPoint07Address,
		activeFrom: 0n,
	},
	{
		id: 3n,
		address: entryPoint08Address,
		activeFrom: 1n,
		activeBefore: 3n,
	},
	{
		id: 4n,
		address: ethAddress,
		activeFrom: 2n,
	},
];
const PARTICIPANTS: Participant[] = PARTICIPANTS_INFO.map((i) => {
	return { address: i.address, id: i.id };
});

describe("participants", () => {
	it("should correctly evaluate ranges", async () => {
		expect(participantsForEpoch(PARTICIPANTS_INFO, 0n)).toStrictEqual([PARTICIPANTS[0], PARTICIPANTS[1]]);
		expect(participantsForEpoch(PARTICIPANTS_INFO, 1n)).toStrictEqual([
			PARTICIPANTS[0],
			PARTICIPANTS[1],
			PARTICIPANTS[2],
		]);
		expect(participantsForEpoch(PARTICIPANTS_INFO, 2n)).toStrictEqual(PARTICIPANTS);
		expect(participantsForEpoch(PARTICIPANTS_INFO, 3n)).toStrictEqual([
			PARTICIPANTS[0],
			PARTICIPANTS[1],
			PARTICIPANTS[3],
		]);
	});
	it("should not return duplicates", async () => {
		expect(participantsForEpoch([...PARTICIPANTS_INFO, PARTICIPANTS_INFO[0]], 0n)).toStrictEqual([
			PARTICIPANTS[0],
			PARTICIPANTS[1],
		]);
		expect(
			participantsForEpoch([PARTICIPANTS_INFO[3], { id: 4n, address: ethAddress, activeFrom: 3n }], 3n),
		).toStrictEqual([PARTICIPANTS[3]]);
	});
	it("should return sorted", async () => {
		expect(participantsForEpoch(PARTICIPANTS_INFO.toReversed(), 2n)).toStrictEqual(PARTICIPANTS);
	});
});
