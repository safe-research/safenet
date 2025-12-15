import { zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import type { KeyGenSecretSharedEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates } from "../types.js";
import { handleKeyGenSecretShared } from "./secretShares.js";

// --- Test Data ---
const TEST_POINT: FrostPoint = toPoint({
	x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
	y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
});

const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "collecting_shares",
		groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
		nextEpoch: 10n,
		deadline: 30n,
	},
	signing: {},
};

const MACHINE_CONFIG: MachineConfig = {
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
	genesisSalt: zeroHash,
	keyGenTimeout: 25n,
	signingTimeout: 20n,
	blocksPerEpoch: 2n,
};

const EVENT: KeyGenSecretSharedEvent = {
	id: "event_key_gen_secret_shared",
	block: 4n,
	index: 0,
	gid: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
	identifier: 2n,
	share: {
		y: TEST_POINT,
		f: [0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
	},
	completed: true,
};

// --- Tests ---
describe("receiving secret shares", () => {
	it("should not handle event if in unexpected state", async () => {
		const machineStates: MachineStates = {
			rollover: { id: "waiting_for_rollover" },
			signing: {},
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not handle event if unexpected group id", async () => {
		const event: KeyGenSecretSharedEvent = {
			...EVENT,
			gid: "0x5afe01",
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, event);

		expect(diff).toStrictEqual({});
	});

	it("should only update last participant if not completed", async () => {
		const event: KeyGenSecretSharedEvent = {
			...EVENT,
			completed: false,
		};
		const handleKeygenSecrets = vi.fn();
		const keyGenClient = {
			handleKeygenSecrets,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, event);

		expect(diff).toStrictEqual({
			rollover: {
				...MACHINE_STATES.rollover,
				lastParticipant: 2n,
			},
		});
		expect(handleKeygenSecrets).toBeCalledTimes(1);
		expect(handleKeygenSecrets).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
		);
	});

	it.skip("should only trigger key gen confirm if valid shares have been submitted", async () => {});

	it("should trigger key gen confirm without callback when doing genesis key gen", async () => {
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 0n,
				deadline: 30n,
			},
			signing: {},
		};
		const handleKeygenSecrets = vi.fn();
		const keyGenClient = {
			handleKeygenSecrets,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, machineStates, EVENT);

		// TODO we should have multiple timeouts (raise complaint, respond to complaint, confirm)
		// Currently only one is returned, the "confirm" timeout, therefore 3x key gen timeout
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 0n,
				deadline: 79n, // 4n (block) + 3n * 25n (key gen timeout)
				lastParticipant: EVENT.identifier,
				sharesFrom: [],
			},
			actions: [
				{
					id: "key_gen_confirm",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					callbackContext: undefined,
				},
			],
		});
		expect(handleKeygenSecrets).toBeCalledTimes(1);
		expect(handleKeygenSecrets).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
		);
	});

	it("should trigger key gen confirm with callback", async () => {
		const handleKeygenSecrets = vi.fn();
		const keyGenClient = {
			handleKeygenSecrets,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, EVENT);

		// TODO we should have multiple timeouts (raise complaint, respond to complaint, confirm)
		// Currently only one is returned, the "confirm" timeout, therefore 3x key gen timeout
		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 79n, // 4n (block) + 3n * 25n (key gen timeout)
				lastParticipant: EVENT.identifier,
				sharesFrom: [],
			},
			actions: [
				{
					id: "key_gen_confirm",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					callbackContext:
						"0x000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000014",
				},
			],
		});
		expect(handleKeygenSecrets).toBeCalledTimes(1);
		expect(handleKeygenSecrets).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
		);
	});
});
