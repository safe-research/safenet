import { describe, expect, it, vi } from "vitest";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { EpochRolloverPacket } from "../../consensus/verify/rollover/schemas.js";
import { toPoint } from "../../frost/math.js";
import type { EpochStagedEvent } from "../transitions/types.js";
import type { MachineStates, SigningState } from "../types.js";
import { handleEpochStaged } from "./epochStaged.js";

// --- Test Data ---
const PACKET: EpochRolloverPacket = {
	type: "epoch_rollover_packet",
	domain: {
		chain: 1n,
		consensus: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
	},
	rollover: {
		activeEpoch: 1n,
		proposedEpoch: 2n,
		rolloverBlock: 20n,
		groupKeyX: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
		groupKeyY: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
	},
};
const INVALID_SIGNING_STATE: SigningState = {
	id: "waiting_for_request",
	signers: [1n, 2n],
	responsible: undefined,
	deadline: 23n,
	packet: PACKET,
};
const SIGNING_STATE: SigningState = {
	id: "waiting_for_attestation",
	signatureId: "0x5af35af3",
	deadline: 22n,
	packet: PACKET,
};

// By default we setup in a genesis state
// This avoids that nonce commitments are triggered every time
const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "sign_rollover",
		groupId: "0x5afe5af3",
		nextEpoch: 2n,
		message: "0x5afe5afe",
		responsible: 1n,
	},
	signing: {
		"0x5afe5afe": SIGNING_STATE,
	},
};

const EVENT: EpochStagedEvent = {
	id: "event_epoch_staged",
	block: 2n,
	index: 0,
	activeEpoch: 1n,
	proposedEpoch: 2n,
	rolloverBlock: 20n,
	groupKey: toPoint({
		x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
		y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
	}),
};

// --- Tests ---
describe("epoch staged", () => {
	it("should throw if in unexpected rollover state", async () => {
		const signingClient: SigningClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			rollover: { id: "waiting_for_rollover" },
			signing: {},
		};
		await expect(handleEpochStaged(signingClient, machineStates, EVENT)).rejects.toStrictEqual(
			new Error("Not expecting epoch staging during waiting_for_rollover!"),
		);
	});

	it("should not handle epoch staged event if in unexpected signing state", async () => {
		const signingClient: SigningClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: { "0x5afe5afe": INVALID_SIGNING_STATE },
		};
		const diff = await handleEpochStaged(signingClient, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should clean up states and trigger nonce commitments after staging rollover", async () => {
		const generateNonceTree = vi.fn();
		generateNonceTree.mockReturnValueOnce("0xdeadb055");
		const signingClient: SigningClient = {
			generateNonceTree,
		} as unknown as SigningClient;
		const diff = await handleEpochStaged(signingClient, MACHINE_STATES, EVENT);
		expect(diff.actions).toStrictEqual([
			{
				id: "sign_register_nonce_commitments",
				groupId: "0x5afe5af3",
				nonceCommitmentsHash: "0xdeadb055",
			},
		]);
		expect(diff.rollover).toStrictEqual({ id: "waiting_for_rollover" });
		expect(diff.consensus).toStrictEqual({
			stagedEpoch: 2n,
			groupPendingNonces: ["0x5afe5af3", true],
			signatureIdToMessage: ["0x5af35af3"],
		});
		expect(diff.signing).toStrictEqual(["0x5afe5afe"]);
	});
});
