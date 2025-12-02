import { zeroHash } from "viem";
import { describe, expect, it } from "vitest";
import type { ConsensusState, MachineStates, RolloverState, SigningState, StateDiff } from "../types.js";
import { LocalConsensusStates, LocalMachineStates } from "./local.js";

// --- Test Data ---
const SIGNING_STATE: SigningState = {
	id: "waiting_for_request",
	signers: [],
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
			rolloverBlock: 23n,
			groupKeyX: 0n,
			groupKeyY: 0n,
		},
	},
};

// --- Tests ---
describe("LocalMachineStates", () => {
	it("should return original state", () => {
		const immutableState: MachineStates = {
			rollover: {
				id: "waiting_for_rollover",
			},
			signing: {
				"0x5afe5afe": SIGNING_STATE,
			},
		};
		const localState = new LocalMachineStates(immutableState);

		expect(localState.rollover).toStrictEqual({ id: "waiting_for_rollover" });
		expect(localState.signing["0x5afe5afe"]).toStrictEqual(SIGNING_STATE);
	});

	it("should return undefined for deleted state", () => {
		const immutableState: MachineStates = {
			rollover: {
				id: "waiting_for_rollover",
			},
			signing: {
				"0x5afe5afe": SIGNING_STATE,
			},
		};
		const localState = new LocalMachineStates(immutableState);
		const diff: StateDiff = {
			signing: ["0x5afe5afe"],
		};
		localState.apply(diff);

		expect(localState.signing["0x5afe5afe"]).toBeUndefined();
		expect(immutableState.signing["0x5afe5afe"]).toStrictEqual(SIGNING_STATE);
	});

	it("should return updated state", () => {
		const immutableState: MachineStates = {
			rollover: {
				id: "waiting_for_rollover",
			},
			signing: {
				"0x5afe5afe": SIGNING_STATE,
			},
		};
		const localState = new LocalMachineStates(immutableState);
		const updatedRollover: RolloverState = {
			id: "collecting_commitments",
			groupId: "0x5afe5afe13",
			nextEpoch: 7n,
			deadline: 11n,
		};
		const updatedState: SigningState = {
			...SIGNING_STATE,
			signatureId: "0x5afe5afe23",
			id: "waiting_for_attestation",
		};
		const diff: StateDiff = {
			rollover: updatedRollover,
			signing: ["0x5afe5afe", updatedState],
		};
		localState.apply(diff);

		expect(localState.rollover).toStrictEqual(updatedRollover);
		expect(localState.signing["0x5afe5afe"]).toStrictEqual(updatedState);
		expect(immutableState.rollover).toStrictEqual({
			id: "waiting_for_rollover",
		});
		expect(immutableState.signing["0x5afe5afe"]).toStrictEqual(SIGNING_STATE);
	});
});

describe("LocalConsensusStates", () => {
	it("should return original state", () => {
		const immutableState: ConsensusState = {
			activeEpoch: 7n,
			stagedEpoch: 11n,
			groupPendingNonces: {
				"0x5afe5afe": true,
			},
			epochGroups: {
				"0x5afe5af3": { groupId: "0x5afe5afe23", participantId: 1n },
			},
			signatureIdToMessage: {
				"0x5afe5afe23": zeroHash,
			},
		};
		const localState = new LocalConsensusStates(immutableState);

		expect(localState.activeEpoch).toBe(7n);
		expect(localState.stagedEpoch).toBe(11n);
		expect(localState.groupPendingNonces["0x5afe5afe"]).toBeTruthy();
		expect(localState.epochGroups["0x5afe5af3"]).toStrictEqual({
			groupId: "0x5afe5afe23",
			participantId: 1n,
		});
		expect(localState.signatureIdToMessage["0x5afe5afe23"]).toBe(zeroHash);
	});

	it("should return undefined for deleted state", () => {
		const immutableState: ConsensusState = {
			activeEpoch: 7n,
			stagedEpoch: 11n,
			groupPendingNonces: {
				"0x5afe5afe": true,
			},
			epochGroups: {
				"0x5afe5af3": { groupId: "0x5afe5afe23", participantId: 1n },
			},
			signatureIdToMessage: {
				"0x5afe5afe23": zeroHash,
			},
		};
		const localState = new LocalConsensusStates(immutableState);
		const diff: StateDiff = {
			consensus: {
				groupPendingNonces: ["0x5afe5afe"],
				signatureIdToMessage: ["0x5afe5afe23"],
			},
		};
		localState.apply(diff);

		expect(localState.groupPendingNonces["0x5afe5afe"]).toBeUndefined();
		expect(localState.signatureIdToMessage["0x5afe5afe23"]).toBeUndefined();

		// Check that immutable state was not touched
		expect(immutableState.activeEpoch).toBe(7n);
		expect(immutableState.stagedEpoch).toBe(11n);
		expect(immutableState.groupPendingNonces["0x5afe5afe"]).toBeTruthy();
		expect(immutableState.epochGroups["0x5afe5af3"]).toStrictEqual({
			groupId: "0x5afe5afe23",
			participantId: 1n,
		});
		expect(immutableState.signatureIdToMessage["0x5afe5afe23"]).toBe(zeroHash);
	});

	it("should return updated state", () => {
		const immutableState: ConsensusState = {
			activeEpoch: 7n,
			stagedEpoch: 11n,
			groupPendingNonces: {
				"0x5afe5afe": true,
			},
			epochGroups: {
				"0x5afe5af3": { groupId: "0x5afe5afe23", participantId: 1n },
			},
			signatureIdToMessage: {
				"0x5afe5afe23": zeroHash,
			},
		};
		const localState = new LocalConsensusStates(immutableState);
		const _updatedState: ConsensusState = {
			activeEpoch: 11n,
			stagedEpoch: 13n,
			groupPendingNonces: {
				"0x5afe5afe": true,
				"0x5afe5afe2": true,
			},
			epochGroups: {
				"0x5afe5af3": { groupId: "0x5afe5afe23", participantId: 1n },
				"0x5afe5af5": { groupId: "0x5afe5afe27", participantId: 1n },
			},
			signatureIdToMessage: {
				"0x5afe5afe23": zeroHash,
				"0x5afe5afe27": zeroHash,
			},
		};
		const diff: StateDiff = {
			consensus: {
				activeEpoch: 11n,
				stagedEpoch: 13n,
				groupPendingNonces: ["0x5afe5afe2", true],
				epochGroup: [0x5afe5af5n, { groupId: "0x5afe5afe27", participantId: 1n }],
				signatureIdToMessage: ["0x5afe5afe27", zeroHash],
			},
		};
		localState.apply(diff);

		expect(localState.activeEpoch).toBe(11n);
		expect(localState.stagedEpoch).toBe(13n);
		expect(localState.groupPendingNonces["0x5afe5afe"]).toBeTruthy();
		expect(localState.groupPendingNonces["0x5afe5afe2"]).toBeTruthy();
		expect(localState.epochGroups["0x5afe5af3"]).toStrictEqual({
			groupId: "0x5afe5afe23",
			participantId: 1n,
		});
		expect(localState.epochGroups[(0x5afe5af5).toString()]).toStrictEqual({
			groupId: "0x5afe5afe27",
			participantId: 1n,
		});
		expect(localState.signatureIdToMessage["0x5afe5afe23"]).toBe(zeroHash);
		expect(localState.signatureIdToMessage["0x5afe5afe27"]).toBe(zeroHash);

		// Check that immutable state was not touched
		expect(immutableState.activeEpoch).toBe(7n);
		expect(immutableState.stagedEpoch).toBe(11n);
		expect(immutableState.groupPendingNonces["0x5afe5afe"]).toBeTruthy();
		expect(immutableState.epochGroups["0x5afe5af3"]).toStrictEqual({
			groupId: "0x5afe5afe23",
			participantId: 1n,
		});
		expect(immutableState.signatureIdToMessage["0x5afe5afe23"]).toBe(zeroHash);
	});
});
