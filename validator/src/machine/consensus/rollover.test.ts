import { ethAddress, zeroAddress, zeroHash } from "viem";
import { describe, expect, it, vi } from "vitest";
import { makeGroupSetup, makeKeyGenSetup } from "../../__tests__/data/machine.js";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { ConsensusState, MachineConfig, MachineStates, SigningState } from "../types.js";
import { checkEpochRollover } from "./rollover.js";

// --- Test Data ---
const MACHINE_CONFIG: MachineConfig = {
	account: "0x0000000000000000000000000000000000000001",
	participantsInfo: [
		{
			address: "0x0000000000000000000000000000000000000001",
			activeFrom: 0n,
		},
		{
			address: "0x0000000000000000000000000000000000000002",
			activeFrom: 0n,
		},
		{
			address: "0x0000000000000000000000000000000000000003",
			activeFrom: 1n,
		},
	],
	genesisSalt: zeroHash,
	keyGenTimeout: 20n,
	signingTimeout: 0n,
	blocksPerEpoch: 10n,
};

const PARTICIPANTS = MACHINE_CONFIG.participantsInfo.map((i) => i.address);

// By default we setup in a genesis state
// This avoids that nonce commitments are triggered every time
const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "waiting_for_genesis",
	},
	signing: {},
};

const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	groupPendingNonces: {},
	epochGroups: {},
	signatureIdToMessage: {},
};

const GROUP_SETUP = makeGroupSetup();
const KEYGEN_SETUP = makeKeyGenSetup();

const EMPTY_PROTOCOL = {} as unknown as SafenetProtocol;
const EMPTY_KEY_GEN_CLIENT = {} as unknown as KeyGenClient;

const makeProtocol = (): SafenetProtocol =>
	({ consensus: vi.fn().mockReturnValueOnce(ethAddress) }) as unknown as SafenetProtocol;

const makeKeyGenClient = (): KeyGenClient =>
	({
		setupGroup: vi.fn().mockReturnValueOnce(GROUP_SETUP),
		setupKeyGen: vi.fn().mockReturnValueOnce(KEYGEN_SETUP),
	}) as unknown as KeyGenClient;

// --- Tests ---
describe("check rollover", () => {
	it("should not trigger key gen in genesis state", async () => {
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			EMPTY_PROTOCOL,
			EMPTY_KEY_GEN_CLIENT,
			CONSENSUS_STATE,
			MACHINE_STATES,
			1n,
		);

		expect(diff).toStrictEqual({});
	});

	it("should not abort genesis key gen", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "collecting_commitments",
				groupId: "0xda5afe3",
				nextEpoch: 0n,
				deadline: 22n,
			},
		};
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			EMPTY_PROTOCOL,
			EMPTY_KEY_GEN_CLIENT,
			CONSENSUS_STATE,
			machineStates,
			1n,
		);

		expect(diff).toStrictEqual({});
	});

	it("should not abort genesis key gen in skipped state (this is an expected halt condition)", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 0n,
			},
		};
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			EMPTY_PROTOCOL,
			EMPTY_KEY_GEN_CLIENT,
			CONSENSUS_STATE,
			machineStates,
			1n,
		);

		expect(diff).toStrictEqual({});
	});

	it("should mark next epoch as skipped when skipping genesis", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "skip_genesis",
			},
		};

		expect(
			checkEpochRollover(MACHINE_CONFIG, EMPTY_PROTOCOL, EMPTY_KEY_GEN_CLIENT, CONSENSUS_STATE, machineStates, 1n),
		).toStrictEqual({
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 1n,
			},
		});

		expect(
			checkEpochRollover(MACHINE_CONFIG, EMPTY_PROTOCOL, EMPTY_KEY_GEN_CLIENT, CONSENSUS_STATE, machineStates, 123n),
		).toStrictEqual({
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 13n,
			},
		});
	});

	it("should not trigger key gen if next epoch is still in the future", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "collecting_commitments",
				groupId: "0xda5afe3",
				nextEpoch: 2n,
				deadline: 22n,
			},
		};
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			EMPTY_PROTOCOL,
			EMPTY_KEY_GEN_CLIENT,
			CONSENSUS_STATE,
			machineStates,
			1n,
		);

		expect(diff).toStrictEqual({});
	});

	it("should not trigger key gen if current epoch was skipped", async () => {
		const machineState: MachineStates = {
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 2n,
			},
			signing: {},
		};
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			EMPTY_PROTOCOL,
			EMPTY_KEY_GEN_CLIENT,
			CONSENSUS_STATE,
			machineState,
			19n,
		);

		expect(diff).toStrictEqual({});
	});

	it("should trigger key gen if previous epoch was skipped", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
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
				participants: GROUP_SETUP.participantsRoot,
				count: 3,
				threshold: 2,
				context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000003",
				commitments: KEYGEN_SETUP.commitments,
				encryptionPublicKey: KEYGEN_SETUP.encryptionPublicKey,
				pok: KEYGEN_SETUP.pok,
				poap: KEYGEN_SETUP.poap,
			},
		]);
		expect(diff.rollover).toStrictEqual({
			id: "collecting_commitments",
			groupId: "0x5afe02",
			nextEpoch: 3n,
			deadline: 40n,
		});
		expect(diff.consensus).toStrictEqual({});
		expect(diff.signing).toBeUndefined();

		expect(protocol.consensus).toBeCalledTimes(1);
		expect(keyGenClient.setupGroup).toBeCalledTimes(1);
		expect(keyGenClient.setupGroup).toBeCalledWith(
			PARTICIPANTS,
			2,
			"0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000003",
		);
	});

	it("should trigger key gen if key gen was aborted (in progress key gen is for a past epoch)", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
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
				participants: GROUP_SETUP.participantsRoot,
				count: 3,
				threshold: 2,
				context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002",
				commitments: KEYGEN_SETUP.commitments,
				encryptionPublicKey: KEYGEN_SETUP.encryptionPublicKey,
				pok: KEYGEN_SETUP.pok,
				poap: KEYGEN_SETUP.poap,
			},
		]);
		expect(diff.rollover).toStrictEqual({
			id: "collecting_commitments",
			groupId: "0x5afe02",
			nextEpoch: 2n,
			deadline: 30n,
		});
		expect(diff.consensus).toStrictEqual({});
		expect(diff.signing).toBeUndefined();

		expect(protocol.consensus).toBeCalledTimes(1);
		expect(keyGenClient.setupGroup).toBeCalledTimes(1);
		expect(keyGenClient.setupGroup).toBeCalledWith(
			PARTICIPANTS,
			2,
			"0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002",
		);
	});

	it("should trigger key gen when staged epoch becomes active", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: { id: "epoch_staged", nextEpoch: 1n },
		};
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineStates, 10n);

		expect(diff.actions).toStrictEqual([
			{
				id: "key_gen_start",
				participants: GROUP_SETUP.participantsRoot,
				count: 3,
				threshold: 2,
				context: "0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002",
				commitments: KEYGEN_SETUP.commitments,
				encryptionPublicKey: KEYGEN_SETUP.encryptionPublicKey,
				pok: KEYGEN_SETUP.pok,
				poap: KEYGEN_SETUP.poap,
			},
		]);
		expect(diff.rollover).toStrictEqual({
			id: "collecting_commitments",
			groupId: "0x5afe02",
			nextEpoch: 2n,
			deadline: 30n,
		});
		expect(diff.consensus).toStrictEqual({
			activeEpoch: 1n,
		});
		expect(diff.signing).toBeUndefined();

		expect(protocol.consensus).toBeCalledTimes(1);
		expect(keyGenClient.setupGroup).toBeCalledTimes(1);
		expect(keyGenClient.setupGroup).toBeCalledWith(
			PARTICIPANTS,
			2,
			"0x00000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee0000000000000002",
		);
	});

	it("should cleanup old epoch groups on epoch activation with non-sequential epochs", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: { id: "epoch_staged", nextEpoch: 7n },
		};
		// Non-sequential epochs: 1, 2, 5, 7
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 5n,
			epochGroups: {
				"1": "0xgroup1",
				"2": "0xgroup2",
				"5": "0xgroup5",
				"7": "0xgroup7",
			},
		};
		// blocksPerEpoch = 10, so block 70 => currentEpoch = 7
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			protocol,
			keyGenClient,
			consensusStateWithGroups,
			machineStates,
			70n,
		);

		// activeEpoch = 5, no signing sessions → epochCutoff = 5
		// Epochs 1 and 2 should be removed (< 5)
		expect(diff.consensus?.removeEpochGroups).toStrictEqual([1n, 2n]);
		expect(diff.consensus?.activeEpoch).toBe(7n);
	});

	it("should preserve epoch groups referenced by active signing sessions", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
		// Active signing session referencing epoch 2
		const signingState: SigningState = {
			id: "waiting_for_request",
			signers: [zeroAddress],
			deadline: 100n,
			packet: {
				type: "safe_transaction_packet",
				domain: {
					chain: 1n,
					consensus: zeroAddress,
				},
				proposal: {
					epoch: 2n,
					transaction: {
						chainId: 1n,
						safe: zeroAddress,
						to: zeroAddress,
						value: 0n,
						data: "0x",
						operation: 0,
						safeTxGas: 0n,
						baseGas: 0n,
						gasPrice: 0n,
						gasToken: zeroAddress,
						refundReceiver: zeroAddress,
						nonce: 0n,
					},
				},
			},
		};
		const machineStates: MachineStates = {
			rollover: { id: "epoch_staged", nextEpoch: 7n },
			signing: {
				"0xabc": signingState,
			},
		};
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 5n,
			epochGroups: {
				"1": "0xgroup1",
				"2": "0xgroup2",
				"3": "0xgroup3",
				"5": "0xgroup5",
				"7": "0xgroup7",
			},
		};
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			protocol,
			keyGenClient,
			consensusStateWithGroups,
			machineStates,
			70n,
		);

		// activeEpoch = 5, smallestSigningEpoch = 2 → epochCutoff = min(2, 5) = 2
		// Only epoch 1 should be removed (< 2)
		expect(diff.consensus?.removeEpochGroups).toStrictEqual([1n]);
		expect(diff.consensus?.activeEpoch).toBe(7n);
	});

	it("should preserve epoch groups referenced by epoch_rollover_packet signing sessions", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
		// Active signing session with epoch_rollover_packet referencing epoch 3
		const signingState: SigningState = {
			id: "waiting_for_request",
			signers: [zeroAddress],
			deadline: 100n,
			packet: {
				type: "epoch_rollover_packet",
				domain: {
					chain: 1n,
					consensus: zeroAddress,
				},
				rollover: {
					activeEpoch: 3n,
					proposedEpoch: 5n,
					rolloverBlock: 50n,
					groupKeyX: 0n,
					groupKeyY: 0n,
				},
			},
		};
		const machineStates: MachineStates = {
			rollover: { id: "epoch_staged", nextEpoch: 5n },
			signing: {
				"0xdef": signingState,
			},
		};
		// Epochs 1, 2, 3 had successful keygens; epoch 5 keygen succeeded and is staged
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 3n,
			epochGroups: {
				"1": "0xgroup1",
				"2": "0xgroup2",
				"3": "0xgroup3",
				"5": "0xgroup5",
			},
		};
		// blocksPerEpoch = 10, so block 50 => currentEpoch = 5
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			protocol,
			keyGenClient,
			consensusStateWithGroups,
			machineStates,
			50n,
		);

		// activeEpoch = 3, smallestSigningEpoch = 3 (from rollover.activeEpoch) → epochCutoff = min(3, 3) = 3
		// Epochs 1 and 2 should be removed (< 3)
		expect(diff.consensus?.removeEpochGroups).toStrictEqual([1n, 2n]);
		expect(diff.consensus?.activeEpoch).toBe(5n);
	});

	it("should not cleanup when all existing epochs are at or above the activeEpoch cutoff", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: { id: "epoch_staged", nextEpoch: 7n },
		};
		// Only epochs 5 and 7 — activeEpoch = 5, epochCutoff = 5, but nothing < 5
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 5n,
			epochGroups: {
				"5": "0xgroup5",
				"7": "0xgroup7",
			},
		};
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			protocol,
			keyGenClient,
			consensusStateWithGroups,
			machineStates,
			70n,
		);

		// activeEpoch = 5, no signing → epochCutoff = 5, but no epoch < 5 exists
		expect(diff.consensus?.removeEpochGroups).toBeUndefined();
		expect(diff.consensus?.activeEpoch).toBe(7n);
	});

	it("should use activeEpoch as epochCutoff when signing epoch is larger", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
		// Signing session referencing epoch 6, which is > activeEpoch (5)
		const signingState: SigningState = {
			id: "waiting_for_request",
			signers: [zeroAddress],
			deadline: 100n,
			packet: {
				type: "safe_transaction_packet",
				domain: {
					chain: 1n,
					consensus: zeroAddress,
				},
				proposal: {
					epoch: 6n,
					transaction: {
						chainId: 1n,
						safe: zeroAddress,
						to: zeroAddress,
						value: 0n,
						data: "0x",
						operation: 0,
						safeTxGas: 0n,
						baseGas: 0n,
						gasPrice: 0n,
						gasToken: zeroAddress,
						refundReceiver: zeroAddress,
						nonce: 0n,
					},
				},
			},
		};
		const machineStates: MachineStates = {
			rollover: { id: "epoch_staged", nextEpoch: 7n },
			signing: {
				"0xabc": signingState,
			},
		};
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 5n,
			epochGroups: {
				"1": "0xgroup1",
				"3": "0xgroup3",
				"5": "0xgroup5",
				"7": "0xgroup7",
			},
		};
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			protocol,
			keyGenClient,
			consensusStateWithGroups,
			machineStates,
			70n,
		);

		// activeEpoch = 5, signingEpoch = 6, since 6 > 5 → epochCutoff = activeEpoch = 5
		// Epochs 1 and 3 should be removed (< 5)
		expect(diff.consensus?.removeEpochGroups).toStrictEqual([1n, 3n]);
	});

	it("should use the minimum epoch across multiple signing sessions", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
		// Two signing sessions: one at epoch 2, one at epoch 4
		const signingState2: SigningState = {
			id: "waiting_for_request",
			signers: [zeroAddress],
			deadline: 100n,
			packet: {
				type: "safe_transaction_packet",
				domain: {
					chain: 1n,
					consensus: zeroAddress,
				},
				proposal: {
					epoch: 2n,
					transaction: {
						chainId: 1n,
						safe: zeroAddress,
						to: zeroAddress,
						value: 0n,
						data: "0x",
						operation: 0,
						safeTxGas: 0n,
						baseGas: 0n,
						gasPrice: 0n,
						gasToken: zeroAddress,
						refundReceiver: zeroAddress,
						nonce: 0n,
					},
				},
			},
		};
		const signingState4: SigningState = {
			id: "waiting_for_request",
			signers: [zeroAddress],
			deadline: 100n,
			packet: {
				type: "safe_transaction_packet",
				domain: {
					chain: 1n,
					consensus: zeroAddress,
				},
				proposal: {
					epoch: 4n,
					transaction: {
						chainId: 1n,
						safe: zeroAddress,
						to: zeroAddress,
						value: 0n,
						data: "0x",
						operation: 0,
						safeTxGas: 0n,
						baseGas: 0n,
						gasPrice: 0n,
						gasToken: zeroAddress,
						refundReceiver: zeroAddress,
						nonce: 0n,
					},
				},
			},
		};
		const machineStates: MachineStates = {
			rollover: { id: "epoch_staged", nextEpoch: 7n },
			signing: {
				"0xabc": signingState2,
				"0xdef": signingState4,
			},
		};
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 5n,
			epochGroups: {
				"1": "0xgroup1",
				"2": "0xgroup2",
				"3": "0xgroup3",
				"5": "0xgroup5",
				"7": "0xgroup7",
			},
		};
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			protocol,
			keyGenClient,
			consensusStateWithGroups,
			machineStates,
			70n,
		);

		// activeEpoch = 5, smallestSigningEpoch = min(2, 4) = 2 → epochCutoff = min(2, 5) = 2
		// Only epoch 1 should be removed (< 2)
		expect(diff.consensus?.removeEpochGroups).toStrictEqual([1n]);
	});

	it("should not cleanup when there are no previous epochs", async () => {
		const protocol = makeProtocol();
		const keyGenClient = makeKeyGenClient();
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: { id: "epoch_staged", nextEpoch: 1n },
		};
		// Only the activating epoch exists, no previous
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"1": "0xgroup1",
			},
		};
		const diff = checkEpochRollover(
			MACHINE_CONFIG,
			protocol,
			keyGenClient,
			consensusStateWithGroups,
			machineStates,
			10n,
		);

		expect(diff.consensus?.removeEpochGroups).toBeUndefined();
	});
});
