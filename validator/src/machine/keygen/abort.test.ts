import { zeroHash } from "viem";
import { describe, expect, it } from "vitest";
import type { ConsensusState, MachineConfig, MachineStates } from "../types.js";
import { checkKeyGenAbort } from "./abort.js";

// --- Test Data ---
const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "collecting_commitments",
		groupId: "0x5afe5afe",
		nextEpoch: 10n,
		deadline: 22n,
	},
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
	defaultParticipants: [],
	genesisSalt: zeroHash,
	keyGenTimeout: 0n,
	signingTimeout: 20n,
	blocksPerEpoch: 1n,
};

// --- Tests ---
describe("key gen abort", () => {
	it("should not abort if waiting for rollover", () => {
		const machineStates: MachineStates = {
			rollover: { id: "waiting_for_rollover" },
			signing: {},
		};
		const diff = checkKeyGenAbort(MACHINE_CONFIG, CONSENSUS_STATE, machineStates, 10n);

		expect(diff).toStrictEqual({});
	});
	it("should not abort if in genesis setup", () => {
		const consensus: ConsensusState = {
			...CONSENSUS_STATE,
			genesisGroupId: "0x5afe5afe",
		};
		const diff = checkKeyGenAbort(MACHINE_CONFIG, consensus, MACHINE_STATES, 10n);

		expect(diff).toStrictEqual({});
	});

	it("should not abort if rollover can still happen", () => {
		const diff = checkKeyGenAbort(MACHINE_CONFIG, CONSENSUS_STATE, MACHINE_STATES, 9n);
		expect(diff).toStrictEqual({});
	});

	it("should abort key gen when current epoch is next epoch after staged epoch", () => {
		const diff = checkKeyGenAbort(MACHINE_CONFIG, CONSENSUS_STATE, MACHINE_STATES, 10n);
		expect(diff.actions).toBeUndefined();
		expect(diff.rollover).toStrictEqual({ id: "waiting_for_rollover" });
		expect(diff.signing).toBeUndefined();
		expect(diff.consensus).toBeUndefined();
	});

	it("should abort key gen when current epoch is multiple epochs after staged epoch", () => {
		const diff = checkKeyGenAbort(MACHINE_CONFIG, CONSENSUS_STATE, MACHINE_STATES, 40n);
		expect(diff.actions).toBeUndefined();
		expect(diff.rollover).toStrictEqual({ id: "waiting_for_rollover" });
		expect(diff.signing).toBeUndefined();
		expect(diff.consensus).toBeUndefined();
	});
});
