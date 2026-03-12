import { zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import { makeGroupSetup } from "../../__tests__/data/machine.js";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { MachineConfig } from "../types.js";
import { triggerKeyGen } from "./trigger.js";

// --- Test Data ---
const PARTICIPANTS = [
	{
		id: 1n,
		address: entryPoint06Address,
		activeFrom: 0n,
	},
	{
		id: 3n,
		address: entryPoint07Address,
		activeFrom: 0n,
	},
	{
		id: 7n,
		address: entryPoint08Address,
		activeFrom: 0n,
	},
];

const MACHINE_CONFIG = {
	participantsInfo: [
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
			activeFrom: 0n,
		},
	],
} as unknown as MachineConfig;

// --- Tests ---
describe("trigger key gen", () => {
	it("should throw if not enough participants are provided (below hard minimum of 2)", () => {
		const keyGenClient = {} as unknown as KeyGenClient;
		// Only provide 1 participant
		expect(triggerKeyGen(MACHINE_CONFIG, keyGenClient, 1n, 20n, PARTICIPANTS.slice(0, 1), zeroHash)).toStrictEqual({
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 1n,
			},
		});
	});

	it("should throw if not enough participants are provided (below crash fault tolerance)", () => {
		const keyGenClient = {} as unknown as KeyGenClient;
		const config = {
			participantsInfo: [
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
					activeFrom: 0n,
				},
				{
					id: 4n,
					address: entryPoint08Address,
					activeFrom: 0n,
				},
			],
		} as unknown as MachineConfig;
		expect(triggerKeyGen(config, keyGenClient, 1n, 20n, PARTICIPANTS.slice(0, 2), zeroHash)).toStrictEqual({
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 1n,
			},
		});
	});

	it("should trigger key generation and return the correct state diff", () => {
		const context = "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002";
		const setupGroup = vi.fn();
		const groupSetup = makeGroupSetup(3n);
		setupGroup.mockReturnValueOnce(groupSetup);
		const keyGenClient = {
			setupGroup,
		} as unknown as KeyGenClient;
		const diff = triggerKeyGen(MACHINE_CONFIG, keyGenClient, 2n, 30n, PARTICIPANTS, context);

		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: groupSetup.participantsRoot,
				count: 3,
				threshold: 2,
				context,
				participantId: 3n,
				commitments: groupSetup.commitments,
				encryptionPublicKey: groupSetup.encryptionPublicKey,
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
		expect(setupGroup).toBeCalledWith(PARTICIPANTS, 2, context);
	});
});
