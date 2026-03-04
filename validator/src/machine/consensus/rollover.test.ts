import { ethAddress, zeroAddress, zeroHash } from "viem";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import type { ConsensusState, MachineConfig, MachineStates, SigningState } from "../types.js";
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

// --- Tests ---
describe("check rollover", () => {
	it("should not trigger key gen in genesis state", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const diff = checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, MACHINE_STATES, 1n);

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

	it("should mark next epoch as skipped when skipping genesis", async () => {
		const protocol = {} as unknown as SafenetProtocol;
		const keyGenClient = {} as unknown as KeyGenClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "skip_genesis",
			},
		};

		expect(
			checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineStates, 1n),
		).toStrictEqual({
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 1n,
			},
		});

		expect(
			checkEpochRollover(MACHINE_CONFIG, protocol, keyGenClient, CONSENSUS_STATE, machineStates, 123n),
		).toStrictEqual({
			rollover: {
				id: "epoch_skipped",
				nextEpoch: 13n,
			},
		});
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
			encryptionPublicKey: TEST_POINT,
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
				encryptionPublicKey: groupSetup.encryptionPublicKey,
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
			encryptionPublicKey: TEST_POINT,
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
			encryptionPublicKey: TEST_POINT,
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

	it("should cleanup old epoch groups on epoch activation with non-sequential epochs", async () => {
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
			encryptionPublicKey: TEST_POINT,
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
			rollover: { id: "epoch_staged", nextEpoch: 7n },
		};
		// Non-sequential epochs: 1, 2, 5, 7
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 5n,
			epochGroups: {
				"1": { groupId: "0xgroup1", participantId: 1n },
				"2": { groupId: "0xgroup2", participantId: 1n },
				"5": { groupId: "0xgroup5", participantId: 1n },
				"7": { groupId: "0xgroup7", participantId: 1n },
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
			encryptionPublicKey: TEST_POINT,
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
		// Active signing session referencing epoch 2
		const signingState: SigningState = {
			id: "waiting_for_request",
			signers: [1n],
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
				"1": { groupId: "0xgroup1", participantId: 1n },
				"2": { groupId: "0xgroup2", participantId: 1n },
				"3": { groupId: "0xgroup3", participantId: 1n },
				"5": { groupId: "0xgroup5", participantId: 1n },
				"7": { groupId: "0xgroup7", participantId: 1n },
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
			encryptionPublicKey: TEST_POINT,
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
		// Active signing session with epoch_rollover_packet referencing epoch 3
		const signingState: SigningState = {
			id: "waiting_for_request",
			signers: [1n],
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
				"1": { groupId: "0xgroup1", participantId: 1n },
				"2": { groupId: "0xgroup2", participantId: 1n },
				"3": { groupId: "0xgroup3", participantId: 1n },
				"5": { groupId: "0xgroup5", participantId: 1n },
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
			encryptionPublicKey: TEST_POINT,
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
			rollover: { id: "epoch_staged", nextEpoch: 7n },
		};
		// Only epochs 5 and 7 — activeEpoch = 5, epochCutoff = 5, but nothing < 5
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 5n,
			epochGroups: {
				"5": { groupId: "0xgroup5", participantId: 1n },
				"7": { groupId: "0xgroup7", participantId: 1n },
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
			encryptionPublicKey: TEST_POINT,
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
		// Signing session referencing epoch 6, which is > activeEpoch (5)
		const signingState: SigningState = {
			id: "waiting_for_request",
			signers: [1n],
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
				"1": { groupId: "0xgroup1", participantId: 1n },
				"3": { groupId: "0xgroup3", participantId: 1n },
				"5": { groupId: "0xgroup5", participantId: 1n },
				"7": { groupId: "0xgroup7", participantId: 1n },
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
			encryptionPublicKey: TEST_POINT,
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
		// Two signing sessions: one at epoch 2, one at epoch 4
		const signingState2: SigningState = {
			id: "waiting_for_request",
			signers: [1n],
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
			signers: [1n],
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
				"1": { groupId: "0xgroup1", participantId: 1n },
				"2": { groupId: "0xgroup2", participantId: 1n },
				"3": { groupId: "0xgroup3", participantId: 1n },
				"5": { groupId: "0xgroup5", participantId: 1n },
				"7": { groupId: "0xgroup7", participantId: 1n },
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
			encryptionPublicKey: TEST_POINT,
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
		// Only the activating epoch exists, no previous
		const consensusStateWithGroups: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"1": { groupId: "0xgroup1", participantId: 1n },
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
