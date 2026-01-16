import { describe, expect, it } from "vitest";
import type { SignatureShareEvent } from "../transitions/types.js";
import type { ConsensusState, MachineStates, SigningState } from "../types.js";
import { handleSigningShares } from "./shares.js";

// --- Test Data ---
const SIGNING_STATE: SigningState = {
	id: "collect_signing_shares",
	signatureId: "0x5af35af3",
	sharesFrom: [],
	lastSigner: undefined,
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

const EVENT: SignatureShareEvent = {
	id: "event_signature_share",
	block: 2n,
	index: 0,
	sid: "0x5af35af3",
	identifier: 1n,
	z: 1n,
	root: "0x5af35af35af35af3000000000000000000000000000000000000000000000000",
};

// --- Tests ---
describe("collecting shares", () => {
	it("should not handle signing requests without a message", async () => {
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			signatureIdToMessage: {},
		};
		const diff = await handleSigningShares(consensusState, MACHINE_STATES, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not handle completed for unknown message", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {},
		};
		const diff = await handleSigningShares(CONSENSUS_STATE, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not handle signing shares when not collecting shares", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					id: "collect_nonce_commitments",
				},
			},
		};
		const diff = await handleSigningShares(CONSENSUS_STATE, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should stay in state and update", async () => {
		const diff = await handleSigningShares(CONSENSUS_STATE, MACHINE_STATES, EVENT);

		expect(diff.consensus).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.actions).toBeUndefined();
		expect(diff.signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "collect_signing_shares",
				signatureId: "0x5af35af3",
				deadline: 23n,
				lastSigner: 1n,
				sharesFrom: [1n],
				packet: SIGNING_STATE.packet,
			},
		]);
		// Check that our original state was not touched
		expect(SIGNING_STATE.sharesFrom).toStrictEqual([]);
	});
});
