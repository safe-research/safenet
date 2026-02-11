import { zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import type { KeyGenSecretSharedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates } from "../types.js";
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
		missingSharesFrom: [],
		complaints: {},
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
	shared: true,
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

	it("should not handle event if not part of group", async () => {
		const participantId = vi.fn();
		participantId.mockImplementationOnce(() => {
			throw new Error("Test Error: unknown group!");
		});
		const keyGenClient = {
			participantId,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, EVENT);

		expect(diff).toStrictEqual({});
		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
	});

	it("should only update last participant if not completed", async () => {
		const event: KeyGenSecretSharedEvent = {
			...EVENT,
			shared: false,
		};
		const participantId = vi.fn();
		const handleKeygenSecrets = vi.fn();
		handleKeygenSecrets.mockReturnValue("pending_shares");
		const keyGenClient = {
			participantId,
			handleKeygenSecrets,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, event);

		expect(diff).toStrictEqual({
			rollover: {
				...MACHINE_STATES.rollover,
				lastParticipant: 2n,
			},
			actions: [],
		});
		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(handleKeygenSecrets).toBeCalledTimes(1);
		expect(handleKeygenSecrets).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
		);
	});

	it("should track who submitted invalid shares", async () => {
		const event: KeyGenSecretSharedEvent = {
			...EVENT,
			shared: false,
		};
		const participantId = vi.fn();
		const handleKeygenSecrets = vi.fn();
		handleKeygenSecrets.mockReturnValue("invalid_share");
		const keyGenClient = {
			participantId,
			handleKeygenSecrets,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, event);

		expect(diff).toStrictEqual({
			rollover: {
				...MACHINE_STATES.rollover,
				missingSharesFrom: [2n],
				lastParticipant: 2n,
			},
			actions: [
				{
					id: "key_gen_complain",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					accused: 2n,
				},
			],
		});
		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(handleKeygenSecrets).toBeCalledTimes(1);
		expect(handleKeygenSecrets).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
		);
	});

	it("should track invalid shares have been submitted and proceed to key gen without sending confirmation", async () => {
		const participantId = vi.fn();
		const handleKeygenSecrets = vi.fn();
		handleKeygenSecrets.mockReturnValue("invalid_share");
		const keyGenClient = {
			participantId,
			handleKeygenSecrets,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, EVENT);

		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 29n, // 4n (block) + 25n (key gen timeout)
				responseDeadline: 54n, // 4n (block) + 2n * 25n (key gen timeout)
				deadline: 79n, // 4n (block) + 3n * 25n (key gen timeout)
				lastParticipant: EVENT.identifier,
				complaints: {},
				missingSharesFrom: [2n],
				confirmationsFrom: [],
			},
			actions: [
				{
					id: "key_gen_complain",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					accused: 2n,
				},
			],
		});
		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(handleKeygenSecrets).toBeCalledTimes(1);
		expect(handleKeygenSecrets).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
		);
	});

	it("should trigger key gen confirm without callback when doing genesis key gen", async () => {
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 0n,
				deadline: 30n,
				missingSharesFrom: [],
				complaints: {},
			},
			signing: {},
		};
		const participantId = vi.fn();
		const handleKeygenSecrets = vi.fn();
		handleKeygenSecrets.mockReturnValue("shares_completed");
		const keyGenClient = {
			participantId,
			handleKeygenSecrets,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, machineStates, EVENT);

		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 0n,
				complaintDeadline: 29n, // 4n (block) + 25n (key gen timeout)
				responseDeadline: 54n, // 4n (block) + 2n * 25n (key gen timeout)
				deadline: 79n, // 4n (block) + 3n * 25n (key gen timeout)
				lastParticipant: EVENT.identifier,
				complaints: {},
				missingSharesFrom: [],
				confirmationsFrom: [],
			},
			actions: [
				{
					id: "key_gen_confirm",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					callbackContext: undefined,
				},
			],
		});
		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(handleKeygenSecrets).toBeCalledTimes(1);
		expect(handleKeygenSecrets).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
		);
	});

	it("should carry over complaints and missing shares", async () => {
		const participantId = vi.fn();
		const handleKeygenSecrets = vi.fn();
		handleKeygenSecrets.mockReturnValue("pending_shares");
		const keyGenClient = {
			participantId,
			handleKeygenSecrets,
		} as unknown as KeyGenClient;

		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 30n,
				missingSharesFrom: [1n],
				complaints: {
					"1": { total: 1n, unresponded: 1n },
				},
			},
			signing: {},
		};

		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, machineStates, EVENT);

		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 29n, // 4n (block) + 25n (key gen timeout)
				responseDeadline: 54n, // 4n (block) + 2n * 25n (key gen timeout)
				deadline: 79n, // 4n (block) + 3n * 25n (key gen timeout)
				lastParticipant: EVENT.identifier,
				missingSharesFrom: [1n],
				complaints: {
					"1": { total: 1n, unresponded: 1n },
				},
				confirmationsFrom: [],
			},
			actions: [],
		});
		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(handleKeygenSecrets).toBeCalledTimes(1);
		expect(handleKeygenSecrets).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
		);
	});

	it("should trigger key gen confirm with callback", async () => {
		const participantId = vi.fn();
		const handleKeygenSecrets = vi.fn();
		handleKeygenSecrets.mockReturnValue("shares_completed");
		const keyGenClient = {
			participantId,
			handleKeygenSecrets,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenSecretShared(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, EVENT);

		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 29n, // 4n (block) + 25n (key gen timeout)
				responseDeadline: 54n, // 4n (block) + 2n * 25n (key gen timeout)
				deadline: 79n, // 4n (block) + 3n * 25n (key gen timeout)
				lastParticipant: EVENT.identifier,
				complaints: {},
				missingSharesFrom: [],
				confirmationsFrom: [],
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
		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(handleKeygenSecrets).toBeCalledTimes(1);
		expect(handleKeygenSecrets).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[0x5afe5afe5afe01n, 0x5afe5afe5afe02n, 0x5afe5afe5afe03n],
		);
	});
});
