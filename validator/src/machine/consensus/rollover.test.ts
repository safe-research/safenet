import { ethAddress, zeroAddress, zeroHash } from "viem";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import type { ConsensusState, MachineConfig, MachineStates } from "../types.js";
import { checkEpochRollover } from "./rollover.js";

// --- Test Data ---
const TEST_POINT: FrostPoint = toPoint({
	x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
	y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
});

const MACHINE_CONFIG: MachineConfig = {
	defaultParticipants: [
		{
			id: 1n,
			address: zeroAddress,
		},
		{
			id: 3n,
			address: zeroAddress,
		},
		{
			id: 7n,
			address: zeroAddress,
		},
	],
	genesisSalt: zeroHash,
	keyGenTimeout: 20n,
	signingTimeout: 0n,
	blocksPerEpoch: 10n,
};

const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	genesisGroupId: "0x5af35afe",
	groupPendingNonces: {},
	epochGroups: {},
	signatureIdToMessage: {},
};

// By default we setup in a genesis state
// This avoids that nonce commitments are triggered every time
const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "waiting_for_genesis",
	},
	signing: {},
};

// --- Tests ---
describe("check rollover", () => {
	it("should not trigger key gen in genesis state", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const consensus: ConsensusState = {
			...CONSENSUS_STATE,
			genesisGroupId: undefined,
		};
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, consensus, MACHINE_STATES, 1n);

		expect(diff).toStrictEqual({});
	});

	it("should not abort genesis key gen", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "collecting_commitments",
				groupId: "0xda5afe3",
				nextEpoch: 0n,
				deadline: 22n,
			},
		};
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineStates, 1n);

		expect(diff).toStrictEqual({});
	});

	it("should not abort genesis key gen in skipped state (this is an expected halt condition)", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 0n,
			},
		};
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineStates, 1n);

		expect(diff).toStrictEqual({});
	});

	it("should not trigger key gen if next epoch is still in the future", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "collecting_commitments",
				groupId: "0xda5afe3",
				nextEpoch: 2n,
				deadline: 22n,
			},
		};
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineStates, 1n);

		expect(diff).toStrictEqual({});
	});

	it("should not trigger key gen if current epoch was skipped", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineState: MachineStates = {
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 2n,
			},
			signing: {},
		};
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineState, 19n);

		expect(diff).toStrictEqual({});
	});

	it("should trigger key gen if previous epoch was skipped", async () => {
		const consensus = vi.fn();
		consensus.mockReturnValueOnce(ethAddress);
		const protocol = {
			consensus,
		} as unknown as SafenetProtocol;
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
		const machineState: MachineStates = {
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 2n,
			},
			signing: {},
		};
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineState, 20n);

		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: groupSetup.participantsRoot,
				count: 3,
				threshold: 2,
				context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000003",
				participantId: 3n,
				commitments: groupSetup.commitments,
				pok: groupSetup.pok,
				poap: groupSetup.poap,
			},
		]);
		expect(diff.rollover).toStrictEqual({
			id: "collecting_commitments",
			groupId: "0x5afe02",
			nextEpoch: 3n,
			deadline: 40n,
		});
		expect(diff.consensus).toStrictEqual({
			epochGroup: [3n, { groupId: "0x5afe02", participantId: 3n }],
		});
		expect(diff.signing).toBeUndefined();

		expect(consensus).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledWith(
			MACHINE_CONFIG.defaultParticipants,
			2,
			"0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000003",
		);
	});

	it("should trigger key gen if key gen was aborted (in progress key gen is for a past epoch)", async () => {
		const consensus = vi.fn();
		consensus.mockReturnValueOnce(ethAddress);
		const protocol = {
			consensus,
		} as unknown as SafenetProtocol;
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
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "collecting_commitments",
				groupId: "0xda5afe3",
				nextEpoch: 1n,
				deadline: 12n,
			},
		};
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineStates, 10n);

		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: groupSetup.participantsRoot,
				count: 3,
				threshold: 2,
				context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002",
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

		expect(consensus).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledWith(
			MACHINE_CONFIG.defaultParticipants,
			2,
			"0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002",
		);
	});

	it("should trigger key gen after when staged epoch becomes active", async () => {
		const consensus = vi.fn();
		consensus.mockReturnValueOnce(ethAddress);
		const protocol = {
			consensus,
		} as unknown as SafenetProtocol;
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
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: { id: "epoch_staged", nextEpoch: 1n },
		};
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineStates, 10n);

		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: groupSetup.participantsRoot,
				count: 3,
				threshold: 2,
				context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002",
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
			activeEpoch: 1n,
		});
		expect(diff.signing).toBeUndefined();

		expect(consensus).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledWith(
			MACHINE_CONFIG.defaultParticipants,
			2,
			"0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002",
		);
	});
});
