import { maxUint64, zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import type { KeyGenEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates } from "../types.js";
import { handleGenesisKeyGen } from "./genesis.js";

// --- Test Data ---
const TEST_POINT: FrostPoint = toPoint({
	x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
	y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
});

const MACHINE_STATES: MachineStates = {
	rollover: { id: "waiting_for_rollover" },
	signing: {},
};

const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	stagedEpoch: 0n,
	groupPendingNonces: {},
	epochGroups: {},
	signatureIdToMessage: {},
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
	blocksPerEpoch: 1n,
};

const EVENT: KeyGenEvent = {
	id: "event_key_gen",
	block: 4n,
	index: 0,
	gid: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
	participants: "0xc41a668cab547c9e2cfd2975f4b6130877c8500c21820daffba14b24c40c742e",
	count: 3n,
	threshold: 2n,
	context: zeroHash,
};

// --- Tests ---
describe("gensis key gen", () => {
	it("should not trigger genesis key gen when not waiting for rollover", () => {
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_commitments",
				groupId: "0x5afe02",
				nextEpoch: 10n,
				deadline: 30n,
			},
			signing: {},
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const diff = handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, CONSENSUS_STATE, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not trigger genesis key gen when already active", () => {
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 1n,
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const diff = handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, consensusState, MACHINE_STATES, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not trigger genesis key gen when an epoch is staged", () => {
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			stagedEpoch: 1n,
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const diff = handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, consensusState, MACHINE_STATES, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not trigger genesis key gen when event group id does not correspond to calculated group id", () => {
		const keyGenClient = {} as unknown as KeyGenClient;
		const event: KeyGenEvent = {
			...EVENT,
			gid: "0x5afe5afe",
		};
		const diff = handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, CONSENSUS_STATE, MACHINE_STATES, event);
		expect(diff).toStrictEqual({});
	});

	it("should trigger genesis key gen with correct parameters", () => {
		const groupSetup = {
			groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			participantsRoot: "0xc41a668cab547c9e2cfd2975f4b6130877c8500c21820daffba14b24c40c742e",
			participantId: 2n,
			commitments: [TEST_POINT],
			pok: {
				r: TEST_POINT,
				mu: 100n,
			},
			poap: ["0x5afe5afe5afe01"],
		};
		const setupGroup = vi.fn();
		setupGroup.mockReturnValueOnce(groupSetup);
		const keyGenClient = {
			setupGroup,
		} as unknown as KeyGenClient;
		const diff = handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, CONSENSUS_STATE, MACHINE_STATES, EVENT);
		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: groupSetup.participantsRoot,
				count: 3n,
				threshold: 2n,
				context: zeroHash,
				participantId: 2n,
				commitments: groupSetup.commitments,
				pok: groupSetup.pok,
				poap: groupSetup.poap,
			},
		]);
		expect(diff.rollover).toStrictEqual({
			id: "collecting_commitments",
			groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			nextEpoch: 0n,
			deadline: maxUint64,
		});
		expect(diff.consensus).toStrictEqual({
			genesisGroupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			epochGroup: [
				0n,
				{ groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000", participantId: 2n },
			],
		});
		expect(diff.signing).toBeUndefined();
		expect(setupGroup).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledWith(MACHINE_CONFIG.defaultParticipants, 3n, 2n, zeroHash);
	});
});
