import { ethAddress, maxUint64, zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import { TEST_POINT } from "../../__tests__/data/machine.js";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { KeyGenEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates } from "../types.js";
import { handleGenesisKeyGen } from "./genesis.js";

// --- Test Data ---
const MACHINE_STATES: MachineStates = {
	rollover: { id: "waiting_for_genesis" },
	signing: {},
};

const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	groupPendingNonces: {},
	epochGroups: {},
	signatureIdToMessage: {},
};

const PARTICIPANTS_INFO = [
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
	{
		id: 5n,
		address: ethAddress,
		activeFrom: 1n,
	},
];

const PARTICIPANTS = PARTICIPANTS_INFO.map((i) => {
	return { address: i.address, id: i.id };
});

const MACHINE_CONFIG: MachineConfig = {
	participantsInfo: PARTICIPANTS_INFO,
	genesisSalt: zeroHash,
	keyGenTimeout: 25n,
	signingTimeout: 20n,
	blocksPerEpoch: 1n,
};

const EVENT: KeyGenEvent = {
	id: "event_key_gen",
	block: 4n,
	index: 0,
	gid: "0x00a9d1aa438a646139fe8d817f9c9dbb060ee7e2e58f2b100000000000000000",
	participants: "0x78d9152d3ca012af785cf642cd52803acabeaea430964b93136f31f83c7df9d0",
	count: 4,
	threshold: 3,
	context: zeroHash,
};

// --- Tests ---
describe("gensis key gen", () => {
	it("should not trigger genesis key gen when not waiting for rollover", async () => {
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
		const diff = await handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, CONSENSUS_STATE, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not trigger genesis key gen when already active", async () => {
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 1n,
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const diff = await handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, consensusState, MACHINE_STATES, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not trigger genesis key gen when event group id does not correspond to calculated group id", async () => {
		const keyGenClient = {} as unknown as KeyGenClient;
		const event: KeyGenEvent = {
			...EVENT,
			gid: "0x5afe5afe",
		};
		const diff = await handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, CONSENSUS_STATE, MACHINE_STATES, event);
		expect(diff).toStrictEqual({});
	});

	it("should throw if different genesis group id is calculated", async () => {
		const groupSetup = {
			groupId: "0xffa9d1aa438a646139fe8d817f9c9dbb060ee7e2e58f2b100000000000000000",
			participantsRoot: "0x78d9152d3ca012af785cf642cd52803acabeaea430964b93136f31f83c7df9d0",
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
		await expect(
			handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, CONSENSUS_STATE, MACHINE_STATES, EVENT),
		).rejects.toStrictEqual(
			new Error("Unexpected genesis group 0xffa9d1aa438a646139fe8d817f9c9dbb060ee7e2e58f2b100000000000000000"),
		);
		expect(setupGroup).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledWith(PARTICIPANTS.slice(0, 4), 3, zeroHash);
	});

	it("should trigger genesis key gen with correct parameters", async () => {
		const groupSetup = {
			groupId: "0x00a9d1aa438a646139fe8d817f9c9dbb060ee7e2e58f2b100000000000000000",
			participantsRoot: "0x78d9152d3ca012af785cf642cd52803acabeaea430964b93136f31f83c7df9d0",
			participantId: 2n,
			commitments: [TEST_POINT],
			encryptionPublicKey: TEST_POINT,
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
		const diff = await handleGenesisKeyGen(MACHINE_CONFIG, keyGenClient, CONSENSUS_STATE, MACHINE_STATES, EVENT);
		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: groupSetup.participantsRoot,
				count: 4,
				threshold: 3,
				context: zeroHash,
				participantId: 2n,
				commitments: groupSetup.commitments,
				encryptionPublicKey: groupSetup.encryptionPublicKey,
				pok: groupSetup.pok,
				poap: groupSetup.poap,
			},
		]);
		expect(diff.rollover).toStrictEqual({
			id: "collecting_commitments",
			groupId: "0x00a9d1aa438a646139fe8d817f9c9dbb060ee7e2e58f2b100000000000000000",
			nextEpoch: 0n,
			deadline: maxUint64,
		});
		expect(diff.consensus).toStrictEqual({
			genesisGroupId: "0x00a9d1aa438a646139fe8d817f9c9dbb060ee7e2e58f2b100000000000000000",
			epochGroup: [
				0n,
				{ groupId: "0x00a9d1aa438a646139fe8d817f9c9dbb060ee7e2e58f2b100000000000000000", participantId: 2n },
			],
		});
		expect(diff.signing).toBeUndefined();
		expect(setupGroup).toBeCalledTimes(1);
		expect(setupGroup).toBeCalledWith(PARTICIPANTS.slice(0, 4), 3, zeroHash);
	});
});
