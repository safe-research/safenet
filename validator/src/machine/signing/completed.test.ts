import { describe, expect, it } from "vitest";
import type {
	ConsensusState,
	MachineConfig,
	MachineStates,
	SigningState,
} from "../types.js";
import { handleSigningCompleted } from "./completed.js";

// --- Test Data ---
const SIGNING_STATE: SigningState = {
	id: "collect_signing_shares",
	signatureId: "0x5af35af3",
	sharesFrom: [1n, 2n],
	lastSigner: 2n,
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
			rolloverBlock: 23n,
			groupKeyX: 0n,
			groupKeyY: 0n,
		},
	},
};

const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "waiting_for_rollover",
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
	signatureIdToMessage: {
		"0x5af35af3": "0x5afe5afe",
	},
};

const MACHINE_CONFIG: MachineConfig = {
	defaultParticipants: [],
	keyGenTimeout: 0n,
	signingTimeout: 20n,
	blocksPerEpoch: 0n,
};

const EVENT_ARGS = {
	sid: "0x5af35af3",
	signature: {
		z: 1n,
		r: {
			x: 2n,
			y: 3n,
		},
	},
};

// --- Tests ---
describe("signing completed", () => {
	it("should fail on invalid event arguments", async () => {
		await expect(
			handleSigningCompleted(
				MACHINE_CONFIG,
				CONSENSUS_STATE,
				MACHINE_STATES,
				2n,
				{},
			),
		).rejects.toThrow();
	});

	it("should not handle signing requests without a message", async () => {
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			signatureIdToMessage: {},
		};
		const diff = await handleSigningCompleted(
			MACHINE_CONFIG,
			consensusState,
			MACHINE_STATES,
			2n,
			EVENT_ARGS,
		);

		expect(diff).toStrictEqual({});
	});

	it("should not handle completed for unknown message", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {},
		};
		const diff = await handleSigningCompleted(
			MACHINE_CONFIG,
			CONSENSUS_STATE,
			machineStates,
			2n,
			EVENT_ARGS,
		);

		expect(diff).toStrictEqual({});
	});

	it("should not handle completed when not collecting shares", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					id: "collect_nonce_commitments",
				},
			},
		};
		const diff = await handleSigningCompleted(
			MACHINE_CONFIG,
			CONSENSUS_STATE,
			machineStates,
			2n,
			EVENT_ARGS,
		);

		expect(diff).toStrictEqual({});
	});

	it("should throw if last signer from collect state is not available", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					lastSigner: undefined,
				},
			},
		};

		await expect(
			handleSigningCompleted(
				MACHINE_CONFIG,
				CONSENSUS_STATE,
				machineStates,
				2n,
				EVENT_ARGS,
			),
		).rejects.toStrictEqual(Error("Invalid state"));
	});

	it("should correctly transition to waiting for attestation", async () => {
		const diff = await handleSigningCompleted(
			MACHINE_CONFIG,
			CONSENSUS_STATE,
			MACHINE_STATES,
			2n,
			EVENT_ARGS,
		);

		expect(diff.consensus).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.actions).toBeUndefined();
		expect(diff.signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "waiting_for_attestation",
				signatureId: "0x5af35af3",
				deadline: 22n,
				responsible: 2n,
				packet: SIGNING_STATE.packet,
			},
		]);
	});
});
