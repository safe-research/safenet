import { zeroAddress, zeroHash } from "viem";
import { describe, expect, it, vi } from "vitest";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { TransactionProposedEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig } from "../types.js";
import { handleTransactionProposed } from "./transactionProposed.js";

// --- Test Data ---
const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	groupPendingNonces: {},
	epochGroups: {
		"10": { groupId: "0x5af3", participantId: 1n },
	},
	signatureIdToMessage: {},
};

const MACHINE_CONFIG: MachineConfig = {
	defaultParticipants: [
		{
			id: 1n,
			address: zeroAddress,
		},
		{
			id: 3n,
			address: zeroAddress,
		},
		{
			id: 7n,
			address: zeroAddress,
		},
		{
			id: 10n,
			address: zeroAddress,
		},
	],
	genesisSalt: zeroHash,
	keyGenTimeout: 0n,
	signingTimeout: 20n,
	blocksPerEpoch: 0n,
};

const EVENT: TransactionProposedEvent = {
	id: "event_transaction_proposed",
	block: 2n,
	index: 0,
	transactionHash: "0x5af35af3",
	chainId: 1n,
	safe: "0x5afe5afe",
	epoch: 10n,
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
describe("transaction proposed", () => {
	it("should not handle proposed event if epoch group is unknown", async () => {
		const protocol: SafenetProtocol = {} as unknown as SafenetProtocol;
		const verificationEngine: VerificationEngine = {} as unknown as VerificationEngine;
		const signingClient: SigningClient = {} as unknown as SigningClient;
		const consensus = {
			...CONSENSUS_STATE,
			epochGroups: {},
		};
		const diff = await handleTransactionProposed(
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
		const signingClient: SigningClient = {} as unknown as SigningClient;
		const diff = await handleTransactionProposed(
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
			type: "safe_transaction_packet",
			domain: {
				chain: 23n,
				consensus: zeroAddress,
			},
			proposal: {
				epoch: EVENT.epoch,
				transaction: EVENT.transaction,
			},
		});
	});

	it("should transition to waiting for request after verifying transaction", async () => {
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
		const participants = vi.fn();
		// Only use a partial set of the default participants
		participants.mockReturnValue([3n, 7n]);
		const signingClient: SigningClient = {
			participants,
		} as unknown as SigningClient;
		const diff = await handleTransactionProposed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			signingClient,
			CONSENSUS_STATE,
			EVENT,
		);
		const packet = {
			type: "safe_transaction_packet",
			domain: {
				chain: 23n,
				consensus: zeroAddress,
			},
			proposal: {
				epoch: EVENT.epoch,
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
				responsible: undefined,
				packet,
				signers: [3n, 7n],
				deadline: 22n,
			},
		]);
		expect(verify).toBeCalledTimes(1);
		expect(verify).toBeCalledWith(packet);
	});
});
