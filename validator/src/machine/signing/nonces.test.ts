import { encodeFunctionData, ethAddress, zeroAddress, zeroHash } from "viem";
import { describe, expect, it, vi } from "vitest";
import { makeMachineConfig } from "../../__tests__/data/machine.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { OracleTransactionPacket } from "../../consensus/verify/oracleTx/schemas.js";
import { safeTxStructHash } from "../../consensus/verify/safeTx/hashing.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import { toPoint } from "../../frost/math.js";
import { CONSENSUS_FUNCTIONS } from "../../types/abis.js";
import type { NonceCommitmentsEvent } from "../transitions/types.js";
import type { ConsensusState, MachineStates, SigningState } from "../types.js";
import { handleRevealedNonces } from "./nonces.js";

// --- Test Data ---
const SIGNING_STATE: SigningState = {
	id: "collect_nonce_commitments",
	signatureId: "0x000000000000000000000000000000000000000000000000000000005af35af3",
	lastSigner: undefined,
	deadline: 13n,
	packet: {
		type: "epoch_rollover_packet",
		domain: {
			chain: 1n,
			consensus: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
		},
		rollover: {
			activeEpoch: 0n,
			proposedEpoch: 3n,
			rolloverBlock: 24n,
			groupKeyX: 0n,
			groupKeyY: 0n,
		},
	},
};

const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "sign_rollover",
		groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
		message: "0x5afe5afe",
		nextEpoch: 3n,
	},
	signing: {
		"0x5afe5afe": SIGNING_STATE,
	},
};

const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	groupPendingNonces: {},
	epochGroups: {},
	signatureIdToMessage: {
		"0x000000000000000000000000000000000000000000000000000000005af35af3": "0x5afe5afe",
	},
};

const MACHINE_CONFIG = makeMachineConfig({ participantsInfo: [], signingTimeout: 20n, blocksPerEpoch: 8n });

const EVENT: NonceCommitmentsEvent = {
	id: "event_nonce_commitments",
	block: 2n,
	index: 0,
	sid: "0x000000000000000000000000000000000000000000000000000000005af35af3",
	participant: "0x0000000000000000000000000000000000005aFE",
	nonces: {
		d: toPoint({
			x: 8157951670743782207572742157759285246997125817591478561509454646417563755134n,
			y: 56888799465634869784517292721691123160415451366201038719887189136540242661500n,
		}),
		e: toPoint({
			x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
			y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
		}),
	},
};

// --- Tests ---
describe("nonces revealed", () => {
	it("should not handle completed for unknown message", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {},
		};
		const diff = await handleRevealedNonces(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should not handle completed when not collecting nonce commitments", async () => {
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {
				"0x5afe5afe": {
					...SIGNING_STATE,
					sharesFrom: [],
					id: "collect_signing_shares",
				},
			},
		};
		const diff = await handleRevealedNonces(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, EVENT);

		expect(diff).toStrictEqual({});
	});

	it("should stay in current state if not completed", async () => {
		const handleNonceCommitments = vi.fn();
		handleNonceCommitments.mockReturnValueOnce(false);
		const signingClient = {
			handleNonceCommitments,
		} as unknown as SigningClient;
		const diff = await handleRevealedNonces(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, MACHINE_STATES, EVENT);

		expect(handleNonceCommitments).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			"0x0000000000000000000000000000000000005aFE",
			{
				hidingNonceCommitment: EVENT.nonces.d,
				bindingNonceCommitment: EVENT.nonces.e,
			},
		);
		expect(handleNonceCommitments).toBeCalledTimes(1);

		expect(diff.consensus).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.actions).toBeUndefined();
		expect(diff.signing).toStrictEqual([
			"0x5afe5afe",
			{
				...SIGNING_STATE,
				lastSigner: "0x0000000000000000000000000000000000005aFE",
			},
		]);
	});

	it("should transition to collect signing shares when completed (epoch)", async () => {
		const signatureShareData = {
			signersRoot: "0xf00baa23",
			signersProof: ["0xf00baa01"],
			groupCommitment: { x: 1n, y: 2n },
			commitmentShare: { x: 1n, y: 2n },
			signatureShare: 0x5afen,
			lagrangeCoefficient: 5n,
		};
		const handleNonceCommitments = vi.fn();
		handleNonceCommitments.mockReturnValueOnce(true);
		const createSignatureShare = vi.fn();
		createSignatureShare.mockReturnValueOnce(signatureShareData);
		const signingClient = {
			handleNonceCommitments,
			createSignatureShare,
		} as unknown as SigningClient;
		const diff = await handleRevealedNonces(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, MACHINE_STATES, EVENT);

		expect(handleNonceCommitments).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			"0x0000000000000000000000000000000000005aFE",
			{
				hidingNonceCommitment: EVENT.nonces.d,
				bindingNonceCommitment: EVENT.nonces.e,
			},
		);
		expect(handleNonceCommitments).toBeCalledTimes(1);

		expect(createSignatureShare).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			ethAddress,
		);
		expect(createSignatureShare).toBeCalledTimes(1);

		expect(diff.consensus).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.signing).toStrictEqual([
			"0x5afe5afe",
			{
				...SIGNING_STATE,
				id: "collect_signing_shares",
				sharesFrom: [],
				deadline: 22n,
				lastSigner: "0x0000000000000000000000000000000000005aFE",
			},
		]);
		const callbackContext =
			"0xea5eeafa000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000000000180000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e14960000000000000000000000000000000000000000000000000000000000000000";
		expect(diff.actions).toStrictEqual([
			{
				id: "sign_publish_signature_share",
				signatureId: "0x000000000000000000000000000000000000000000000000000000005af35af3",
				callbackContext,
				...signatureShareData,
			},
		]);
	});

	it("should transition to collect signing shares when completed (transaction)", async () => {
		// Set package for a Safe transaction attestation
		const packet: SafeTransactionPacket = {
			type: "safe_transaction_packet",
			domain: {
				chain: 1n,
				consensus: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
			},
			proposal: {
				epoch: 22n,
				transaction: {
					chainId: 0n,
					safe: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
					to: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
					value: 0n,
					data: "0x",
					operation: 0,
					safeTxGas: 0n,
					baseGas: 0n,
					gasPrice: 0n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
					nonce: 0n,
				},
			},
		};
		const signingState: SigningState = {
			...SIGNING_STATE,
			packet,
		};
		// Set the rollover state to a different message from this signing event
		const machineStates: MachineStates = {
			rollover: {
				id: "sign_rollover",
				groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
				message: "0x5afe5af3",
				nextEpoch: 3n,
			},
			signing: {
				"0x5afe5afe": signingState,
			},
		};
		const signatureShareData = {
			signersRoot: "0xf00baa23",
			signersProof: ["0xf00baa01"],
			groupCommitment: { x: 1n, y: 2n },
			commitmentShare: { x: 1n, y: 2n },
			signatureShare: 0x5afen,
			lagrangeCoefficient: 5n,
		};
		const handleNonceCommitments = vi.fn();
		handleNonceCommitments.mockReturnValueOnce(true);
		const createSignatureShare = vi.fn();
		createSignatureShare.mockReturnValueOnce(signatureShareData);
		const signingClient = {
			handleNonceCommitments,
			createSignatureShare,
		} as unknown as SigningClient;
		const diff = await handleRevealedNonces(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, EVENT);

		expect(handleNonceCommitments).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			"0x0000000000000000000000000000000000005aFE",
			{
				hidingNonceCommitment: EVENT.nonces.d,
				bindingNonceCommitment: EVENT.nonces.e,
			},
		);
		expect(handleNonceCommitments).toBeCalledTimes(1);

		expect(createSignatureShare).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			ethAddress,
		);
		expect(createSignatureShare).toBeCalledTimes(1);

		expect(diff.consensus).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.signing).toStrictEqual([
			"0x5afe5afe",
			{
				...signingState,
				id: "collect_signing_shares",
				sharesFrom: [],
				deadline: 22n,
				lastSigner: "0x0000000000000000000000000000000000005aFE",
			},
		]);
		const callbackContext =
			"0xaa8d17390000000000000000000000000000000000000000000000000000000000000016000000000000000000000000000000000000000000000000000000000000000000000000000000000000000089bef0f3a116cf717e51f74c271a0a7af527511dde4e914dcc51de13a527bcebf7c516fe17dd27ee03310485bc7920c16b1f892d0000000000000000000000000000000000000000000000000000000000000000";
		expect(diff.actions).toStrictEqual([
			{
				id: "sign_publish_signature_share",
				signatureId: "0x000000000000000000000000000000000000000000000000000000005af35af3",
				callbackContext,
				...signatureShareData,
			},
		]);
	});

	it("should transition to collect signing shares when completed (oracle transaction)", async () => {
		// Set package for an oracle transaction attestation
		const packet: OracleTransactionPacket = {
			type: "oracle_transaction_packet",
			domain: {
				chain: 1n,
				consensus: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
			},
			proposal: {
				epoch: 22n,
				oracle: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
				transaction: {
					chainId: 0n,
					safe: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
					to: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
					value: 0n,
					data: "0x",
					operation: 0,
					safeTxGas: 0n,
					baseGas: 0n,
					gasPrice: 0n,
					gasToken: zeroAddress,
					refundReceiver: zeroAddress,
					nonce: 0n,
				},
			},
		};
		const signingState: SigningState = {
			...SIGNING_STATE,
			packet,
		};
		// Set the rollover state to a different message from this signing event
		const machineStates: MachineStates = {
			rollover: {
				id: "sign_rollover",
				groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
				message: "0x5afe5af3",
				nextEpoch: 3n,
			},
			signing: {
				"0x5afe5afe": signingState,
			},
		};
		const signatureShareData = {
			signersRoot: "0xf00baa23",
			signersProof: ["0xf00baa01"],
			groupCommitment: { x: 1n, y: 2n },
			commitmentShare: { x: 1n, y: 2n },
			signatureShare: 0x5afen,
			lagrangeCoefficient: 5n,
		};
		const handleNonceCommitments = vi.fn();
		handleNonceCommitments.mockReturnValueOnce(true);
		const createSignatureShare = vi.fn();
		createSignatureShare.mockReturnValueOnce(signatureShareData);
		const signingClient = {
			handleNonceCommitments,
			createSignatureShare,
		} as unknown as SigningClient;
		const diff = await handleRevealedNonces(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, EVENT);

		expect(diff.signing).toStrictEqual([
			"0x5afe5afe",
			{
				...signingState,
				id: "collect_signing_shares",
				sharesFrom: [],
				deadline: 22n,
				lastSigner: "0x0000000000000000000000000000000000005aFE",
			},
		]);
		const { chainId, safe, ...transactionData } = packet.proposal.transaction;
		const expectedCallbackContext = encodeFunctionData({
			abi: CONSENSUS_FUNCTIONS,
			functionName: "attestOracleTransaction",
			args: [packet.proposal.epoch, packet.proposal.oracle, chainId, safe, safeTxStructHash(transactionData), zeroHash],
		});
		expect(diff.actions).toStrictEqual([
			{
				id: "sign_publish_signature_share",
				signatureId: "0x000000000000000000000000000000000000000000000000000000005af35af3",
				callbackContext: expectedCallbackContext,
				...signatureShareData,
			},
		]);
	});

	it("should transition to collect signing shares when completed (unknown)", async () => {
		// Set the rollover state to a different message from this signing event
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			rollover: {
				id: "sign_rollover",
				groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
				message: "0x5afe5af3",
				nextEpoch: 3n,
			},
		};
		const signatureShareData = {
			signersRoot: "0xf00baa23",
			signersProof: ["0xf00baa01"],
			groupCommitment: { x: 1n, y: 2n },
			commitmentShare: { x: 1n, y: 2n },
			signatureShare: 0x5afen,
			lagrangeCoefficient: 5n,
		};
		const handleNonceCommitments = vi.fn();
		handleNonceCommitments.mockReturnValueOnce(true);
		const createSignatureShare = vi.fn();
		createSignatureShare.mockReturnValueOnce(signatureShareData);
		const signingClient = {
			handleNonceCommitments,
			createSignatureShare,
		} as unknown as SigningClient;
		const diff = await handleRevealedNonces(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, machineStates, EVENT);

		expect(handleNonceCommitments).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			"0x0000000000000000000000000000000000005aFE",
			{
				hidingNonceCommitment: EVENT.nonces.d,
				bindingNonceCommitment: EVENT.nonces.e,
			},
		);
		expect(handleNonceCommitments).toBeCalledTimes(1);

		expect(createSignatureShare).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			ethAddress,
		);
		expect(createSignatureShare).toBeCalledTimes(1);

		expect(diff.consensus).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.signing).toStrictEqual([
			"0x5afe5afe",
			{
				...SIGNING_STATE,
				id: "collect_signing_shares",
				sharesFrom: [],
				deadline: 22n,
				lastSigner: "0x0000000000000000000000000000000000005aFE",
			},
		]);
		expect(diff.actions).toStrictEqual([
			{
				id: "sign_publish_signature_share",
				signatureId: "0x000000000000000000000000000000000000000000000000000000005af35af3",
				callbackContext: undefined,
				...signatureShareData,
			},
		]);
	});
});
