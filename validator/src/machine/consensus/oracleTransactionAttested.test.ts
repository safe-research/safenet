import { zeroAddress } from "viem";
import { describe, expect, it } from "vitest";
import { TEST_POINT } from "../../__tests__/data/protocol.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import { oracleTxPacketHash } from "../../consensus/verify/oracleTx/hashing.js";
import type { OracleTransactionPacket } from "../../consensus/verify/oracleTx/schemas.js";
import { safeTxHash } from "../../consensus/verify/safeTx/hashing.js";
import type { OracleTransactionAttestedEvent } from "../transitions/types.js";
import type { MachineStates, SigningState } from "../types.js";
import { handleOracleTransactionAttested } from "./oracleTransactionAttested.js";

// --- Test Data ---
const ORACLE_ADDRESS = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";

const PROTOCOL = {
	chainId: () => 42n,
	consensus: () => "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
} as unknown as SafenetProtocol;

const PACKET: OracleTransactionPacket = {
	type: "oracle_transaction_packet",
	domain: {
		chain: PROTOCOL.chainId(),
		consensus: PROTOCOL.consensus(),
	},
	proposal: {
		epoch: 10n,
		oracle: ORACLE_ADDRESS,
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

const MESSAGE = oracleTxPacketHash(PACKET);

const INVALID_SIGNING_STATE: SigningState = {
	id: "waiting_for_oracle",
	oracle: ORACLE_ADDRESS,
	gid: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
	signatureId: "0x5af35af300000000000000000000000000000000000000000000000000000000",
	sequence: 0n,
	packet: PACKET,
	signers: [zeroAddress],
	deadline: 100n,
};

const SIGNING_STATE: SigningState = {
	id: "waiting_for_attestation",
	signatureId: "0x5af35af3",
	deadline: 22n,
	packet: PACKET,
};

const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "waiting_for_genesis",
	},
	signing: {
		[MESSAGE]: SIGNING_STATE,
	},
};

const EVENT: OracleTransactionAttestedEvent = {
	id: "event_oracle_transaction_attested",
	block: 2n,
	index: 0,
	safeTxHash: safeTxHash(PACKET.proposal.transaction),
	chainId: PACKET.proposal.transaction.chainId,
	safe: PACKET.proposal.transaction.safe,
	epoch: PACKET.proposal.epoch,
	oracle: ORACLE_ADDRESS,
	signatureId: "0x5af35af3",
	attestation: {
		z: 12345n,
		r: TEST_POINT,
	},
};

// --- Tests ---
describe("oracle transaction attested", () => {
	it("should not handle attestation event if in unexpected state", async () => {
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: { [MESSAGE]: INVALID_SIGNING_STATE },
		};
		const diff = await handleOracleTransactionAttested(PROTOCOL, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should clean up states", async () => {
		const diff = await handleOracleTransactionAttested(PROTOCOL, MACHINE_STATES, EVENT);
		expect(diff.actions).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.consensus).toStrictEqual({ signatureIdToMessage: ["0x5af35af3", undefined] });
		expect(diff.signing).toStrictEqual([MESSAGE]);
	});
});
