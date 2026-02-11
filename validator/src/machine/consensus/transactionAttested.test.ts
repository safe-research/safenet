import { zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import { TEST_POINT } from "../../__tests__/data/protocol.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import { safeTxHash, safeTxPacketHash } from "../../consensus/verify/safeTx/hashing.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import type { TransactionAttestedEvent } from "../transitions/types.js";
import type { MachineStates, SigningState } from "../types.js";
import { handleTransactionAttested } from "./transactionAttested.js";

// --- Test Data ---
const PROTOCOL = {
	chainId: () => 42n,
	consensus: () => "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
} as unknown as SafenetProtocol;
const PACKET: SafeTransactionPacket = {
	type: "safe_transaction_packet",
	domain: {
		chain: PROTOCOL.chainId(),
		consensus: PROTOCOL.consensus(),
	},
	proposal: {
		epoch: 10n,
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
			nonce: 2n,
		},
	},
};
const MESSAGE = safeTxPacketHash(PACKET);

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
		id: "waiting_for_genesis",
	},
	signing: {
		[MESSAGE]: SIGNING_STATE,
	},
};

const EVENT: TransactionAttestedEvent = {
	id: "event_transaction_attested",
	block: 2n,
	index: 0,
	transactionHash: safeTxHash(PACKET.proposal.transaction),
	epoch: PACKET.proposal.epoch,
	attestation: {
		z: 12345n,
		r: TEST_POINT,
	},
};

// --- Tests ---
describe("transaction attested", () => {
	it("should not handle attestation event if in unexpected state", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: { [MESSAGE]: INVALID_SIGNING_STATE },
		};
		const diff = await handleTransactionAttested(PROTOCOL, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should clean up states", async () => {
		const diff = await handleTransactionAttested(PROTOCOL, MACHINE_STATES, EVENT);
		const message = safeTxPacketHash(PACKET);
		expect(diff.actions).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.consensus).toStrictEqual({ signatureIdToMessage: ["0x5af35af3"] });
		expect(diff.signing).toStrictEqual([message]);
	});
});
