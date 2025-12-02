import { describe, expect, it, vi } from "vitest";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import type { ConsensusState, MachineConfig, MachineStates, SigningState } from "../types.js";
import { checkSigningTimeouts } from "./timeouts.js";

// --- Test Data ---
const SIGNING_STATE: SigningState = {
	id: "waiting_for_request",
	signers: [1n, 2n, 3n],
	responsible: undefined,
	deadline: 23n,
	packet: {
		type: "epoch_rollover_packet",
		domain: {
			chain: 1n,
			consensus: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
		},
		rollover: {
			activeEpoch: 0n,
			proposedEpoch: 3n,
			rolloverBlock: 24n,
			groupKeyX: 0n,
			groupKeyY: 0n,
		},
	},
};

const TX_ATTESTATION_PACKET: SafeTransactionPacket = {
	type: "safe_transaction_packet",
	domain: {
		chain: 1n,
		consensus: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
	},
	proposal: {
		epoch: 22n,
		transaction: {
			to: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
			value: 0n,
			data: "0x",
			operation: 0,
			nonce: 0n,
			chainId: 0n,
			account: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
		},
	},
};

// By default we setup in a genesis state
// This avoids that nonce commitments are triggered every time
const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "sign_rollover",
		groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
		message: "0x5afe5afe",
		nextEpoch: 3n,
		responsible: 1n,
	},
	signing: {
		"0x5afe5afe": SIGNING_STATE,
	},
};

const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	stagedEpoch: 0n,
	groupPendingNonces: {},
	epochGroups: {},
	signatureIdToMessage: {},
};

const MACHINE_CONFIG: MachineConfig = {
	defaultParticipants: [],
	keyGenTimeout: 0n,
	signingTimeout: 20n,
	blocksPerEpoch: 8n,
};

// --- Tests ---
describe("signing timeouts - base conditions", () => {
	it("should return empty state if there are no signing requests", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n);

		expect(diff).toStrictEqual([]);
	});

	it("should not handle request that are within the deadline", async () => {
		const signingClient = {} as unknown as SigningClient;
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, MACHINE_STATES, 2n);
		expect(diff).toStrictEqual([{}]);
	});
});

describe("signing timeouts - waiting for attestation", () => {
	it("should timeout without action when someone else responsible", async () => {
		const participantId = vi.fn().mockReturnValueOnce(2n);
		const signingClient = {
			participantId,
		} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: SIGNING_STATE.packet,
					deadline: 1n,
					responsible: 1n,
					signatureId: "0x5af35af3",
					id: "waiting_for_attestation",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_attestation",
				deadline: 22n,
				signatureId: "0x5af35af3",
				responsible: undefined,
				packet: SIGNING_STATE.packet,
			},
		]);
		expect(diff[0].actions).toBeUndefined();

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5af35af3");
	});

	it("should timeout with actions when I am responsible (epoch rollover)", async () => {
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			participantId,
		} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: SIGNING_STATE.packet,
					deadline: 1n,
					responsible: 1n,
					signatureId: "0x5af35af3",
					id: "waiting_for_attestation",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_attestation",
				deadline: 22n,
				signatureId: "0x5af35af3",
				responsible: undefined,
				packet: SIGNING_STATE.packet,
			},
		]);
		expect(diff[0].actions).toStrictEqual([
			{
				id: "consensus_stage_epoch",
				groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
				proposedEpoch: 3n,
				rolloverBlock: 24n,
				signatureId: "0x5af35af3",
			},
		]);

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5af35af3");
	});

	it("should timeout with actions when I am responsible (transaction attestation)", async () => {
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			participantId,
		} as unknown as SigningClient;
		// Set package for a Safe transaction attestation
		const signingState: SigningState = {
			packet: TX_ATTESTATION_PACKET,
			deadline: 1n,
			responsible: 1n,
			signatureId: "0x5af35af3",
			id: "waiting_for_attestation",
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe5afe": signingState,
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe5afe",
			{
				id: "waiting_for_attestation",
				deadline: 22n,
				signatureId: "0x5af35af3",
				responsible: undefined,
				packet: signingState.packet,
			},
		]);
		expect(diff[0].actions).toStrictEqual([
			{
				id: "consensus_attest_transaction",
				epoch: 22n,
				transactionHash: "0x916684b071b093f33683e48ed3ee895d9a8cae6d5e6a8a6621a30c306857f6a5",
				signatureId: "0x5af35af3",
			},
		]);

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5af35af3");
	});

	it("should timeout with actions when I am responsible (epoch rollover)", async () => {
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			participantId,
		} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe5afe": {
					packet: SIGNING_STATE.packet,
					deadline: 1n,
					responsible: 1n,
					signatureId: "0x5af35af3",
					id: "waiting_for_attestation",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe5afe",
			{
				id: "waiting_for_attestation",
				deadline: 22n,
				signatureId: "0x5af35af3",
				responsible: undefined,
				packet: SIGNING_STATE.packet,
			},
		]);
		expect(diff[0].actions).toBeUndefined();

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5af35af3");
	});

	it("should timeout with actions when everyone is responsible (epoch rollover)", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: SIGNING_STATE.packet,
					deadline: 1n,
					responsible: undefined,
					signatureId: "0x5af35af3",
					id: "waiting_for_attestation",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual(["0x5afe5afe", undefined]);
		expect(diff[0].actions).toStrictEqual([
			{
				id: "consensus_stage_epoch",
				groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
				proposedEpoch: 3n,
				rolloverBlock: 24n,
				signatureId: "0x5af35af3",
			},
		]);
	});

	it("should timeout with actions when everyone is responsible (transaction attestation)", async () => {
		const signingClient = {} as unknown as SigningClient;
		// Set package for a Safe transaction attestation
		const signingState: SigningState = {
			packet: TX_ATTESTATION_PACKET,
			deadline: 1n,
			responsible: undefined,
			signatureId: "0x5af35af3",
			id: "waiting_for_attestation",
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe5afe": signingState,
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual(["0x5afe5afe5afe", undefined]);
		expect(diff[0].actions).toStrictEqual([
			{
				id: "consensus_attest_transaction",
				epoch: 22n,
				transactionHash: "0x916684b071b093f33683e48ed3ee895d9a8cae6d5e6a8a6621a30c306857f6a5",
				signatureId: "0x5af35af3",
			},
		]);
	});

	it("should timeout with actions when everyone is responsible (unknown packet)", async () => {
		const signingClient = {} as unknown as SigningClient;
		// Set package for a Safe transaction attestation
		const signingState: SigningState = {
			packet: SIGNING_STATE.packet,
			deadline: 1n,
			responsible: undefined,
			signatureId: "0x5af35af3",
			id: "waiting_for_attestation",
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe5afe": signingState,
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual(["0x5afe5afe5afe", undefined]);
		expect(diff[0].actions).toBeUndefined();
	});
});

describe("signing timeouts - waiting for request", () => {
	it("should throw if group is unknown", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					deadline: 1n,
					responsible: 2n,
				},
			},
		};
		expect(() => {
			checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n);
		}).toThrowError("Unknown group for epoch 0");
	});

	it("should timeout without action when someone else responsible (epoch rollover)", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					deadline: 1n,
					responsible: 2n,
				},
			},
		};
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"0": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toBeUndefined();
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				signers: [1n, 3n],
				responsible: undefined,
				packet: SIGNING_STATE.packet,
			},
		]);
		expect(diff[0].actions).toBeUndefined();
	});

	it("should timeout with actions when everyone is responsible (epoch rollover)", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					deadline: 1n,
				},
			},
		};
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"0": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toBeUndefined();
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual(["0x5afe5afe", undefined]);
		expect(diff[0].actions).toStrictEqual([
			{
				id: "sign_request",
				groupId: "0x5afe",
				message: "0x5afe5afe",
			},
		]);
	});

	it("should timeout with actions when I am responsible (epoch rollover)", async () => {
		// TODO: this state makes no sense, when I am responsible then I need to remove myself from the signers,
		// because I fucked up before
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					deadline: 1n,
					responsible: 1n,
				},
			},
		};
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"0": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toBeUndefined();
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				signers: [2n, 3n],
				responsible: undefined,
				packet: SIGNING_STATE.packet,
			},
		]);
		expect(diff[0].actions).toStrictEqual([
			{
				id: "sign_request",
				groupId: "0x5afe",
				message: "0x5afe5afe",
			},
		]);
	});

	it("should timeout without action when someone else responsible (transaction attestation)", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					packet: TX_ATTESTATION_PACKET,
					deadline: 1n,
					responsible: 2n,
				},
			},
		};
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"22": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toBeUndefined();
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				signers: [1n, 3n],
				responsible: undefined,
				packet: TX_ATTESTATION_PACKET,
			},
		]);
		expect(diff[0].actions).toBeUndefined();
	});

	it("should timeout with actions when everyone is responsible (transaction attestation)", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					packet: TX_ATTESTATION_PACKET,
					deadline: 1n,
				},
			},
		};
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"22": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toBeUndefined();
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual(["0x5afe5afe", undefined]);
		expect(diff[0].actions).toStrictEqual([
			{
				id: "sign_request",
				groupId: "0x5afe",
				message: "0x5afe5afe",
			},
		]);
	});

	it("should timeout with actions when I am responsible (transaction attestation)", async () => {
		// TODO: this state makes no sense, when I am responsible then I need to remove myself from the signers,
		// because I fucked up before
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					packet: TX_ATTESTATION_PACKET,
					deadline: 1n,
					responsible: 1n,
				},
			},
		};
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"22": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toBeUndefined();
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				signers: [2n, 3n],
				responsible: undefined,
				packet: TX_ATTESTATION_PACKET,
			},
		]);
		expect(diff[0].actions).toStrictEqual([
			{
				id: "sign_request",
				groupId: "0x5afe",
				message: "0x5afe5afe",
			},
		]);
	});
});
