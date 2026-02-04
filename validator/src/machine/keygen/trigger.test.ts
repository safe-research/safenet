import { zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import type { MachineConfig } from "../types.js";
import { triggerKeyGen } from "./trigger.js";

// --- Test Data ---
const TEST_POINT: FrostPoint = toPoint({
	x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
	y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
});

const PARTICIPANTS = [
	{
		id: 1n,
		address: entryPoint06Address,
	},
	{
		id: 3n,
		address: entryPoint07Address,
	},
	{
		id: 7n,
		address: entryPoint08Address,
	},
];

const MACHINE_CONFIG = {
	defaultParticipants: [
		{
			id: 1n,
			address: entryPoint06Address,
		},
		{
			id: 2n,
			address: entryPoint07Address,
		},
		{
			id: 3n,
			address: entryPoint08Address,
		},
	],
} as unknown as MachineConfig;

// --- Tests ---
describe("trigger key gen", () => {
	it("should throw if not enough participants are provided (below hard minimum of 2)", () => {
		const keyGenClient = {} as unknown as KeyGenClient;
		// Only provide 1 participant
		expect(() => triggerKeyGen(MACHINE_CONFIG, keyGenClient, 1n, 20n, PARTICIPANTS.slice(0, 1), zeroHash)).toThrowError(
			new Error("Not enough participants! Expected at least 3 got 1"),
		);
	});

	it("should throw if not enough participants are provided (below crash fault tolerance)", () => {
		const keyGenClient = {} as unknown as KeyGenClient;
		const config = {
			defaultParticipants: [
				{
					id: 1n,
					address: entryPoint06Address,
				},
				{
					id: 2n,
					address: entryPoint07Address,
				},
				{
					id: 3n,
					address: entryPoint08Address,
				},
				{
					id: 4n,
					address: entryPoint08Address,
				},
			],
		} as unknown as MachineConfig;
		expect(() => triggerKeyGen(config, keyGenClient, 1n, 20n, PARTICIPANTS.slice(0, 2), zeroHash)).toThrowError(
			new Error("Not enough participants! Expected at least 3 got 2"),
		);
	});

	it("should trigger key generation and return the correct state diff", () => {
		const context = "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002";
		const setupGroup = vi.fn();
		const groupSetup = {
			groupId: "0x5afe02",
			participantsRoot: "0x5afe5afe5afe",
			participantId: 3n,
			commitments: [TEST_POINT],
			pok: {
				r: TEST_POINT,
				mu: 100n,
			},
			poap: ["0x5afe5afe5afe01"],
		};
		setupGroup.mockReturnValueOnce(groupSetup);
		const keyGenClient = {
			setupGroup,
		} as unknown as KeyGenClient;
		const { groupId, diff } = triggerKeyGen(MACHINE_CONFIG, keyGenClient, 2n, 30n, PARTICIPANTS, context);

		expect(groupId).toBe("0x5afe02");
		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: groupSetup.participantsRoot,
				count: 3,
				threshold: 2,
				context,
				participantId: 3n,
				commitments: groupSetup.commitments,
				pok: groupSetup.pok,
				poap: groupSetup.poap,
			},
		]);
		expect(diff.rollover).toStrictEqual({
			id: "collecting_commitments",
			groupId: "0x5afe02",
			nextEpoch: 2n,
			deadline: 30n,
		});
		expect(diff.consensus).toStrictEqual({
			epochGroup: [2n, { groupId: "0x5afe02", participantId: 3n }],
		});
		expect(diff.signing).toBeUndefined();

		expect(setupGroup).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledWith(PARTICIPANTS, 3, 2, context);
	});
});
