import { zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import type { KeyGenCommittedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates } from "../types.js";
import { handleKeyGenCommitted } from "./committed.js";

// --- Test Data ---
const TEST_POINT: FrostPoint = toPoint({
	x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
	y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
});

const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "collecting_commitments",
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

const EVENT: KeyGenCommittedEvent = {
	id: "event_key_gen_committed",
	block: 4n,
	index: 0,
	gid: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
	identifier: 2n,
	commitment: {
		r: TEST_POINT,
		mu: 0x5afen,
		c: [TEST_POINT, TEST_POINT, TEST_POINT],
	},
	committed: true,
};

// --- Tests ---
describe("key gen committed", () => {
	it("should not handle event if in unexpected state", async () => {
		const machineStates: MachineStates = {
			rollover: { id: "waiting_for_rollover" },
			signing: {},
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const diff = await handleKeyGenCommitted(MACHINE_CONFIG, keyGenClient, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not handle event if for unexpected group id", async () => {
		const event: KeyGenCommittedEvent = {
			...EVENT,
			gid: "0x5afe01",
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const diff = await handleKeyGenCommitted(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, event);

		expect(diff).toStrictEqual({});
	});

	it("should not update state if not fully committed", async () => {
		const event: KeyGenCommittedEvent = {
			...EVENT,
			committed: false,
		};
		const handleKeygenCommitment = vi.fn();
		const keyGenClient = {
			handleKeygenCommitment,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenCommitted(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, event);

		expect(diff).toStrictEqual({});
		expect(handleKeygenCommitment).toBeCalledTimes(1);
		expect(handleKeygenCommitment).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[TEST_POINT, TEST_POINT, TEST_POINT],
			{
				r: TEST_POINT,
				mu: 0x5afen,
			},
		);
	});

	it("should publish secret shares once fully committed", async () => {
		const handleKeygenCommitment = vi.fn();
		const createSecretShares = vi.fn();
		createSecretShares.mockReturnValueOnce({
			verificationShare: TEST_POINT,
			shares: [0x5afe01n, 0x5afe03n],
		});
		const keyGenClient = {
			handleKeygenCommitment,
			createSecretShares,
		} as unknown as KeyGenClient;
		const diff = await handleKeyGenCommitted(MACHINE_CONFIG, keyGenClient, MACHINE_STATES, EVENT);

		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_shares",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				deadline: 29n,
				complaints: {},
			},
			actions: [
				{
					id: "key_gen_publish_secret_shares",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					verificationShare: TEST_POINT,
					shares: [0x5afe01n, 0x5afe03n],
				},
			],
		});
		expect(handleKeygenCommitment).toBeCalledTimes(1);
		expect(handleKeygenCommitment).toBeCalledWith(
			"0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			2n,
			[TEST_POINT, TEST_POINT, TEST_POINT],
			{
				r: TEST_POINT,
				mu: 0x5afen,
			},
		);
		expect(createSecretShares).toBeCalledTimes(1);
		expect(createSecretShares).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
	});
});
