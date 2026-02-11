import { zeroAddress, zeroHash } from "viem";
import { describe, expect, it, vi } from "vitest";
import { log } from "../../__tests__/config.js";
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
			chainId: 0n,
			safe: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
			to: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
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
};

// By default we setup in a genesis state
// This avoids that nonce commitments are triggered every time
const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "sign_rollover",
		groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
		message: "0x5afe5afe",
		nextEpoch: 3n,
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
	defaultParticipants: [
		{ id: 1n, address: zeroAddress },
		{ id: 2n, address: zeroAddress },
		{ id: 3n, address: zeroAddress },
	],
	genesisSalt: zeroHash,
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);

		expect(diff).toStrictEqual([]);
	});

	it("should not handle request that are within the deadline", async () => {
		const signingClient = {} as unknown as SigningClient;
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, MACHINE_STATES, 2n, log);
		expect(diff).toStrictEqual([{}]);
	});
});

describe("signing timeouts - waiting for attestation", () => {
	it("should timeout without action when someone else responsible", async () => {
		const signingGroup = vi.fn().mockReturnValueOnce("0x5afe");
		const participantId = vi.fn().mockReturnValueOnce(2n);
		const signingClient = {
			signingGroup,
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
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

		expect(signingGroup).toBeCalledTimes(1);
		expect(signingGroup).toBeCalledWith("0x5af35af3");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});

	it("should timeout with actions when I am responsible (epoch rollover)", async () => {
		const signingGroup = vi.fn().mockReturnValueOnce("0x5afe");
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			signingGroup,
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
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

		expect(signingGroup).toBeCalledTimes(1);
		expect(signingGroup).toBeCalledWith("0x5af35af3");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});

	it("should timeout with actions when I am responsible (transaction attestation)", async () => {
		const signingGroup = vi.fn().mockReturnValueOnce("0x5afe");
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			signingGroup,
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
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
				transactionHash: "0xd17a3cccc647b9a43f15b53cd396e7ded1c0bb5dde266146f8f8aa86804f76fe",
				signatureId: "0x5af35af3",
			},
		]);

		expect(signingGroup).toBeCalledTimes(1);
		expect(signingGroup).toBeCalledWith("0x5af35af3");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});

	it("should timeout with actions when I am responsible (epoch rollover)", async () => {
		const signingGroup = vi.fn().mockReturnValueOnce("0x5afe");
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			signingGroup,
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
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

		expect(signingGroup).toBeCalledTimes(1);
		expect(signingGroup).toBeCalledWith("0x5af35af3");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
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
				transactionHash: "0xd17a3cccc647b9a43f15b53cd396e7ded1c0bb5dde266146f8f8aa86804f76fe",
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
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
	it("should drop request if group is unknown (epoch rollover)", async () => {
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
		const diffs = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
		expect(diffs).toStrictEqual([
			{
				signing: ["0x5afe5afe", undefined],
			},
		]);
	});

	it("should drop request if not enought signers (epoch rollover)", async () => {
		const threshold = vi.fn().mockReturnValueOnce(3);
		const signingClient = {
			threshold,
		} as unknown as SigningClient;
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
		const diffs = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diffs).toStrictEqual([
			{
				signing: ["0x5afe5afe", undefined],
			},
		]);

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");
	});

	it("should timeout without action when someone else responsible (epoch rollover)", async () => {
		const threshold = vi.fn().mockReturnValueOnce(2);
		const signingClient = {
			threshold,
		} as unknown as SigningClient;
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
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

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");
	});

	it("should timeout with actions when everyone is responsible (epoch rollover)", async () => {
		const threshold = vi.fn().mockReturnValueOnce(2);
		const signingClient = {
			threshold,
		} as unknown as SigningClient;
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
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

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");
	});

	it("should timeout with actions when I am responsible (epoch rollover)", async () => {
		const threshold = vi.fn().mockReturnValueOnce(2);
		const signingClient = {
			threshold,
		} as unknown as SigningClient;
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
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

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");
	});

	it("should drop request if group is unknown (transaction attestation)", async () => {
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
		const diffs = checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
		expect(diffs).toStrictEqual([
			{
				signing: ["0x5afe5afe", undefined],
			},
		]);
	});

	it("should drop request if not enought signers (transaction attestation)", async () => {
		const threshold = vi.fn().mockReturnValueOnce(3);
		const signingClient = {
			threshold,
		} as unknown as SigningClient;
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
		const diffs = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diffs).toStrictEqual([
			{
				signing: ["0x5afe5afe", undefined],
			},
		]);

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");
	});

	it("should timeout without action when someone else responsible (transaction attestation)", async () => {
		const threshold = vi.fn().mockReturnValueOnce(2);
		const signingClient = {
			threshold,
		} as unknown as SigningClient;
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
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

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");
	});

	it("should timeout with actions when everyone is responsible (transaction attestation)", async () => {
		const threshold = vi.fn().mockReturnValueOnce(2);
		const signingClient = {
			threshold,
		} as unknown as SigningClient;
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
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

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");
	});

	it("should timeout with actions when I am responsible (transaction attestation)", async () => {
		const threshold = vi.fn().mockReturnValueOnce(2);
		const signingClient = {
			threshold,
		} as unknown as SigningClient;
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
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
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

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");
	});
});

describe("signing timeouts - collect nonce commitments", () => {
	it("should throw if group is unknown", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const missingNonces = vi.fn().mockReturnValueOnce([3n]);
		const signingClient = {
			missingNonces,
			signers,
		} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					id: "collect_nonce_commitments",
				},
			},
		};
		expect(() => {
			checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
		}).toThrowError("Unknown group for epoch 0");

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(missingNonces).toBeCalledTimes(1);
		expect(missingNonces).toBeCalledWith("0x5af35af3");
	});

	it("should delete signing request if not enought signers for retry", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const missingNonces = vi.fn().mockReturnValueOnce([2n, 3n]);
		const signingClient = {
			missingNonces,
			signers,
			threshold,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"0": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					id: "collect_nonce_commitments",
				},
			},
		};
		const diffs = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diffs).toStrictEqual([
			{
				consensus: {
					signatureIdToMessage: ["0x5af35af3", undefined],
				},
				signing: ["0x5afe5afe", undefined],
			},
		]);

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");

		expect(missingNonces).toBeCalledTimes(1);
		expect(missingNonces).toBeCalledWith("0x5af35af3");
	});

	it("should timeout without action when someone else responsible (epoch rollover)", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const missingNonces = vi.fn().mockReturnValueOnce([3n]);
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			participantId,
			missingNonces,
			signers,
			threshold,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"0": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: SIGNING_STATE.packet,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					id: "collect_nonce_commitments",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				responsible: 2n,
				signers: [1n, 2n],
				packet: SIGNING_STATE.packet,
			},
		]);
		expect(diff[0].actions).toBeUndefined();

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");

		expect(missingNonces).toBeCalledTimes(1);
		expect(missingNonces).toBeCalledWith("0x5af35af3");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});

	it("should timeout without action when I am responsible (epoch rollover)", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const missingNonces = vi.fn().mockReturnValueOnce([3n]);
		const participantId = vi.fn().mockReturnValueOnce(2n);
		const signingClient = {
			participantId,
			missingNonces,
			signers,
			threshold,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"0": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: SIGNING_STATE.packet,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					id: "collect_nonce_commitments",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				responsible: 2n,
				signers: [1n, 2n],
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

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");

		expect(missingNonces).toBeCalledTimes(1);
		expect(missingNonces).toBeCalledWith("0x5af35af3");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});

	it("should timeout without action when someone else responsible (transaction attestation)", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const missingNonces = vi.fn().mockReturnValueOnce([3n]);
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			participantId,
			missingNonces,
			signers,
			threshold,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"22": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: TX_ATTESTATION_PACKET,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					id: "collect_nonce_commitments",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				responsible: 2n,
				signers: [1n, 2n],
				packet: TX_ATTESTATION_PACKET,
			},
		]);
		expect(diff[0].actions).toBeUndefined();

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");

		expect(missingNonces).toBeCalledTimes(1);
		expect(missingNonces).toBeCalledWith("0x5af35af3");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});

	it("should timeout without action when I am responsible (transaction attestation)", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const missingNonces = vi.fn().mockReturnValueOnce([3n]);
		const participantId = vi.fn().mockReturnValueOnce(2n);
		const signingClient = {
			participantId,
			missingNonces,
			signers,
			threshold,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"22": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: TX_ATTESTATION_PACKET,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					id: "collect_nonce_commitments",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				responsible: 2n,
				signers: [1n, 2n],
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

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");

		expect(missingNonces).toBeCalledTimes(1);
		expect(missingNonces).toBeCalledWith("0x5af35af3");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});
});

describe("signing timeouts - collect signing shares", () => {
	it("should throw if group is unknown", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const signingClient = {
			signers,
		} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					sharesFrom: [1n, 2n],
					id: "collect_signing_shares",
				},
			},
		};
		expect(() => {
			checkSigningTimeouts(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, 2n, log);
		}).toThrowError("Unknown group for epoch 0");

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");
	});

	it("should delete signing request if not enought signers for retry", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const signingClient = {
			signers,
			threshold,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"0": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					sharesFrom: [2n],
					id: "collect_signing_shares",
				},
			},
		};
		const diffs = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diffs).toStrictEqual([
			{
				consensus: {
					signatureIdToMessage: ["0x5af35af3", undefined],
				},
				signing: ["0x5afe5afe", undefined],
			},
		]);

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");
	});

	it("should timeout without action when someone else responsible (epoch rollover)", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			participantId,
			signers,
			threshold,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"0": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: SIGNING_STATE.packet,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					sharesFrom: [1n, 2n],
					id: "collect_signing_shares",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				responsible: 2n,
				signers: [1n, 2n],
				packet: SIGNING_STATE.packet,
			},
		]);
		expect(diff[0].actions).toBeUndefined();

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});

	it("should timeout without action when I am responsible (epoch rollover)", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const participantId = vi.fn().mockReturnValueOnce(2n);
		const signingClient = {
			participantId,
			signers,
			threshold,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"0": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: SIGNING_STATE.packet,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					sharesFrom: [1n, 2n],
					id: "collect_signing_shares",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				responsible: 2n,
				signers: [1n, 2n],
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

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});

	it("should timeout without action when someone else responsible (transaction attestation)", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const participantId = vi.fn().mockReturnValueOnce(1n);
		const signingClient = {
			participantId,
			signers,
			threshold,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"22": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: TX_ATTESTATION_PACKET,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					sharesFrom: [1n, 2n],
					id: "collect_signing_shares",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				responsible: 2n,
				signers: [1n, 2n],
				packet: TX_ATTESTATION_PACKET,
			},
		]);
		expect(diff[0].actions).toBeUndefined();

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});

	it("should timeout without action when I am responsible (transaction attestation)", async () => {
		const signers = vi.fn().mockReturnValueOnce([1n, 2n, 3n]);
		const threshold = vi.fn().mockReturnValueOnce(2);
		const participantId = vi.fn().mockReturnValueOnce(2n);
		const signingClient = {
			threshold,
			participantId,
			signers,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			epochGroups: {
				"22": { groupId: "0x5afe", participantId: 1n },
			},
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					packet: TX_ATTESTATION_PACKET,
					deadline: 1n,
					lastSigner: 2n,
					signatureId: "0x5af35af3",
					sharesFrom: [1n, 2n],
					id: "collect_signing_shares",
				},
			},
		};
		const diff = checkSigningTimeouts(MACHINE_CONFIG, signingClient, consensusState, machineStates, 2n, log);
		expect(diff.length).toBe(1);
		expect(diff[0].consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", undefined],
		});
		expect(diff[0].rollover).toBeUndefined();
		expect(diff[0].signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_request",
				deadline: 22n,
				responsible: 2n,
				signers: [1n, 2n],
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

		expect(signers).toBeCalledTimes(1);
		expect(signers).toBeCalledWith("0x5af35af3");

		expect(threshold).toBeCalledTimes(1);
		expect(threshold).toBeCalledWith("0x5afe");

		expect(participantId).toBeCalledTimes(1);
		expect(participantId).toBeCalledWith("0x5afe");
	});
});
