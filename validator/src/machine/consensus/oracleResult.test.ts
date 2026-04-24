import { zeroAddress } from "viem";
import { describe, expect, it, vi } from "vitest";
import { makeMachineConfig } from "../../__tests__/data/machine.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import { oracleTxPacketHash } from "../../consensus/verify/oracleTx/hashing.js";
import type { OracleTransactionPacket } from "../../consensus/verify/oracleTx/schemas.js";
import type { OracleResultEvent } from "../transitions/types.js";
import type { MachineStates, SigningState } from "../types.js";
import { handleOracleResult } from "./oracleResult.js";

// --- Test Data ---
const ORACLE_ADDRESS = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const GROUP_ID = "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496";
const SIGNATURE_ID = "0x5af35af300000000000000000000000000000000000000000000000000000000";

const PACKET: OracleTransactionPacket = {
	type: "oracle_transaction_packet",
	domain: {
		chain: 1n,
		consensus: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
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

const REQUEST_ID = oracleTxPacketHash(PACKET);

const WAITING_FOR_ORACLE_STATE: SigningState = {
	id: "waiting_for_oracle",
	oracle: ORACLE_ADDRESS,
	gid: GROUP_ID,
	signatureId: SIGNATURE_ID,
	sequence: 5n,
	signers: [zeroAddress, "0x0000000000000000000000000000000000000001"],
	deadline: 100n,
	packet: PACKET,
};

const MACHINE_STATES: MachineStates = {
	rollover: { id: "waiting_for_genesis" },
	signing: {
		[REQUEST_ID]: WAITING_FOR_ORACLE_STATE,
	},
};

const MACHINE_CONFIG = makeMachineConfig({ signingTimeout: 20n });

const APPROVED_EVENT: OracleResultEvent = {
	id: "event_oracle_result",
	block: 50n,
	index: 0,
	requestId: REQUEST_ID,
	proposer: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
	result: "0x",
	approved: true,
};

const REJECTED_EVENT: OracleResultEvent = {
	...APPROVED_EVENT,
	approved: false,
};

// --- Tests ---
describe("oracle result", () => {
	it("should not handle oracle result if no signing state exists", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = { ...MACHINE_STATES, signing: {} };
		const diff = await handleOracleResult(MACHINE_CONFIG, signingClient, machineStates, APPROVED_EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should not handle oracle result if state is not waiting_for_oracle", async () => {
		const signingClient = {} as unknown as SigningClient;
		const wrongState: SigningState = {
			id: "waiting_for_request",
			signers: [zeroAddress],
			responsible: undefined,
			deadline: 100n,
			packet: PACKET,
		};
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: { [REQUEST_ID]: wrongState },
		};
		const diff = await handleOracleResult(MACHINE_CONFIG, signingClient, machineStates, APPROVED_EVENT);
		expect(diff).toStrictEqual({});
	});

	it("should drop state when oracle rejects", async () => {
		const signingClient = {} as unknown as SigningClient;
		const diff = await handleOracleResult(MACHINE_CONFIG, signingClient, MACHINE_STATES, REJECTED_EVENT);
		expect(diff.actions).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.consensus).toBeUndefined();
		expect(diff.signing).toStrictEqual([REQUEST_ID]);
	});

	it("should transition to collect_nonce_commitments when oracle approves", async () => {
		const createNonceCommitments = vi.fn().mockReturnValueOnce({
			nonceCommitments: "0xaabb",
			nonceProof: ["0xcc"],
		});
		const signingClient = { createNonceCommitments } as unknown as SigningClient;
		const diff = await handleOracleResult(MACHINE_CONFIG, signingClient, MACHINE_STATES, APPROVED_EVENT);

		expect(createNonceCommitments).toBeCalledTimes(1);
		expect(createNonceCommitments).toBeCalledWith(
			GROUP_ID,
			MACHINE_CONFIG.account,
			SIGNATURE_ID,
			REQUEST_ID,
			5n,
			WAITING_FOR_ORACLE_STATE.signers,
		);

		expect(diff.rollover).toBeUndefined();
		expect(diff.consensus).toStrictEqual({ signatureIdToMessage: [SIGNATURE_ID, REQUEST_ID] });
		expect(diff.signing).toStrictEqual([
			REQUEST_ID,
			{
				id: "collect_nonce_commitments",
				signatureId: SIGNATURE_ID,
				deadline: 70n, // block(50) + signingTimeout(20)
				lastSigner: undefined,
				packet: PACKET,
			},
		]);
		expect(diff.actions).toStrictEqual([
			{
				id: "sign_reveal_nonce_commitments",
				signatureId: SIGNATURE_ID,
				nonceCommitments: "0xaabb",
				nonceProof: ["0xcc"],
			},
		]);
	});
});
