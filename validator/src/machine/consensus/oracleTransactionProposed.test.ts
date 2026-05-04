import { zeroAddress } from "viem";
import { describe, expect, it, vi } from "vitest";
import { makeMachineConfig } from "../../__tests__/data/machine.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { OracleTransactionProposedEvent } from "../transitions/types.js";
import type { ConsensusState } from "../types.js";
import { handleOracleTransactionProposed } from "./oracleTransactionProposed.js";

// --- Test Data ---
const ORACLE_ADDRESS = "0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97";

const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	groupPendingNonces: {},
	epochGroups: {
		"10": "0x5af3",
	},
	signatureIdToMessage: {},
};

const MACHINE_CONFIG = makeMachineConfig({ signingTimeout: 20n, allowedOracles: [ORACLE_ADDRESS], oracleTimeout: 30n });

const EVENT: OracleTransactionProposedEvent = {
	id: "event_oracle_transaction_proposed",
	block: 2n,
	index: 0,
	safeTxHash: "0x5af35af3",
	chainId: 1n,
	safe: "0x5afe5afe",
	epoch: 10n,
	oracle: ORACLE_ADDRESS,
	transaction: {
		chainId: 1n,
		safe: "0x5afe5afe",
		to: "0x5afe5afe",
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
};

// --- Tests ---
describe("oracle transaction proposed", () => {
	it("should not handle proposed event if not part of signing group", async () => {
		const protocol: SafenetProtocol = {} as unknown as SafenetProtocol;
		const verificationEngine: VerificationEngine = {} as unknown as VerificationEngine;
		const hasParticipant = vi.fn().mockReturnValueOnce(false);
		const signingClient: SigningClient = {
			hasParticipant,
		} as unknown as SigningClient;
		const diff = await handleOracleTransactionProposed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			signingClient,
			CONSENSUS_STATE,
			EVENT,
		);

		expect(diff).toStrictEqual({});
	});

	it("should not handle proposed event if epoch group is unknown", async () => {
		const protocol: SafenetProtocol = {} as unknown as SafenetProtocol;
		const verificationEngine: VerificationEngine = {} as unknown as VerificationEngine;
		const hasParticipant = vi.fn().mockReturnValueOnce(true);
		const signingClient: SigningClient = {
			hasParticipant,
		} as unknown as SigningClient;
		const consensus = {
			...CONSENSUS_STATE,
			epochGroups: {},
		};
		const diff = await handleOracleTransactionProposed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			signingClient,
			consensus,
			EVENT,
		);

		expect(diff).toStrictEqual({});
	});

	it("should not update state if message cannot be verified", async () => {
		const protocol: SafenetProtocol = {
			chainId: () => 23n,
			consensus: () => zeroAddress,
		} as unknown as SafenetProtocol;
		const verify = vi.fn();
		verify.mockResolvedValueOnce({
			status: "invalid",
			error: new Error("Test Verification Error"),
		});
		const verificationEngine: VerificationEngine = {
			verify,
		} as unknown as VerificationEngine;
		const hasParticipant = vi.fn().mockReturnValueOnce(true);
		const signingClient: SigningClient = {
			hasParticipant,
		} as unknown as SigningClient;
		const diff = await handleOracleTransactionProposed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			signingClient,
			CONSENSUS_STATE,
			EVENT,
		);
		expect(diff).toStrictEqual({});
		expect(verify).toBeCalledTimes(1);
		expect(verify).toBeCalledWith({
			type: "oracle_transaction_packet",
			domain: {
				chain: 23n,
				consensus: zeroAddress,
			},
			proposal: {
				epoch: EVENT.epoch,
				oracle: EVENT.oracle,
				transaction: EVENT.transaction,
			},
		});
	});

	it("should transition to waiting_for_request after verifying oracle transaction", async () => {
		const protocol: SafenetProtocol = {
			chainId: () => 23n,
			consensus: () => zeroAddress,
		} as unknown as SafenetProtocol;
		const verify = vi.fn();
		verify.mockReturnValue({
			status: "valid",
			packetId: "0x5af35afe",
		});
		const verificationEngine: VerificationEngine = {
			verify,
		} as unknown as VerificationEngine;
		const hasParticipant = vi.fn().mockReturnValueOnce(true);
		const participants = vi.fn();
		participants.mockReturnValue([3n, 7n]);
		const signingClient: SigningClient = {
			hasParticipant,
			participants,
		} as unknown as SigningClient;
		const diff = await handleOracleTransactionProposed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			signingClient,
			CONSENSUS_STATE,
			EVENT,
		);
		const packet = {
			type: "oracle_transaction_packet",
			domain: {
				chain: 23n,
				consensus: zeroAddress,
			},
			proposal: {
				epoch: EVENT.epoch,
				oracle: EVENT.oracle,
				transaction: EVENT.transaction,
			},
		};
		expect(diff.actions).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.consensus).toBeUndefined();
		expect(diff.signing).toStrictEqual([
			"0x5af35afe",
			{
				id: "waiting_for_request",
				packet,
				signers: [3n, 7n],
				deadline: 22n, // block(2) + signingTimeout(20)
			},
		]);
		expect(verify).toBeCalledTimes(1);
		expect(verify).toBeCalledWith(packet);
	});
});
