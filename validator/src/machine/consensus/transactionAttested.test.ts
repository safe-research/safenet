import { describe, expect, it } from "vitest";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import type { TransactionAttestedEvent } from "../transitions/types.js";
import type { MachineStates, SigningState } from "../types.js";
import { handleTransactionAttested } from "./transactionAttested.js";

// --- Test Data ---
const PACKET: SafeTransactionPacket = {
	type: "safe_transaction_packet",
	domain: {
		chain: 1n,
		consensus: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
	},
	proposal: {
		epoch: 10n,
		transaction: {
			to: "0x5afe5afe",
			value: 0n,
			data: "0x",
			operation: 0,
			nonce: 2n,
			chainId: 1n,
			account: "0x5afe5afe",
		},
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
		id: "waiting_for_rollover",
	},
	signing: {
		"0x5afe5afe": SIGNING_STATE,
	},
};

const EVENT: TransactionAttestedEvent = {
	id: "event_transaction_attested",
	block: 2n,
	index: 0,
	message: "0x5afe5afe",
};

// --- Tests ---
describe("transaction attested", () => {
	it("should not handle attestation event if in unexpected state", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: { "0x5afe5afe": INVALID_SIGNING_STATE },
		};
		const diff = await handleTransactionAttested(machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should clean up states", async () => {
		const diff = await handleTransactionAttested(MACHINE_STATES, EVENT);
		expect(diff.actions).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.consensus).toStrictEqual({ signatureIdToMessage: ["0x5af35af3"] });
		expect(diff.signing).toStrictEqual(["0x5afe5afe"]);
	});
});
