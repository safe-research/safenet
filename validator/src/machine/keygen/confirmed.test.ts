import { keccak256, zeroHash } from "viem";
import { entryPoint06Address, entryPoint07Address, entryPoint08Address } from "viem/account-abstraction";
import { describe, expect, it, vi } from "vitest";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { EpochRolloverPacket } from "../../consensus/verify/rollover/schemas.js";
import { toPoint } from "../../frost/math.js";
import type { FrostPoint } from "../../frost/types.js";
import { jsonReplacer } from "../../utils/json.js";
import type { KeyGenConfirmedEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates, RolloverState, SigningState } from "../types.js";
import { handleKeyGenConfirmed } from "./confirmed.js";

// --- Test Data ---
const TEST_POINT: FrostPoint = toPoint({
	x: 73844941487532555987364396775795076447946974313865618280135872376303125438365n,
	y: 29462187596282402403443212507099371496473451788807502182979305411073244917417n,
});

const EPOCH_PACKET: EpochRolloverPacket = {
	type: "epoch_rollover_packet",
	domain: {
		chain: 100n,
		consensus: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
	},
	rollover: {
		activeEpoch: 0n,
		proposedEpoch: 10n,
		rolloverBlock: 20n,
		groupKeyX: TEST_POINT.x,
		groupKeyY: TEST_POINT.y,
	},
};

const MACHINE_STATES: MachineStates = {
	rollover: {
		id: "collecting_confirmations",
		groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
		nextEpoch: 10n,
		complaintDeadline: 20n,
		responseDeadline: 25n,
		deadline: 30n,
		missingSharesFrom: [],
		confirmationsFrom: [3n, 1n],
		complaints: {},
	},
	signing: {},
};

const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	groupPendingNonces: {},
	epochGroups: {},
	signatureIdToMessage: {},
};

const MACHINE_CONFIG: MachineConfig = {
	defaultParticipants: [
		{
			id: 1n,
			address: entryPoint06Address,
		},
		{
			id: 2n,
			address: entryPoint07Address,
		},
		{
			id: 3n,
			address: entryPoint08Address,
		},
	],
	genesisSalt: zeroHash,
	keyGenTimeout: 25n,
	signingTimeout: 20n,
	blocksPerEpoch: 2n,
};

const EVENT: KeyGenConfirmedEvent = {
	id: "event_key_gen_confirmed",
	block: 4n,
	index: 0,
	gid: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
	identifier: 2n,
	confirmed: false,
};

// --- Tests ---
describe("key gen confirmed", () => {
	it("should not handle event if in unexpected state", async () => {
		const machineStates: MachineStates = {
			rollover: { id: "waiting_for_genesis" },
			signing: {},
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const signingClient = {} as unknown as SigningClient;
		const protocol = {} as unknown as SafenetProtocol;
		const verificationEngine = {} as unknown as VerificationEngine;
		const diff = await handleKeyGenConfirmed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			keyGenClient,
			signingClient,
			CONSENSUS_STATE,
			machineStates,
			EVENT,
		);

		expect(diff).toStrictEqual({});
	});

	it("should not handle event if for unexpected group id", async () => {
		const event: KeyGenConfirmedEvent = {
			...EVENT,
			gid: "0x5afe01",
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const signingClient = {} as unknown as SigningClient;
		const protocol = {} as unknown as SafenetProtocol;
		const verificationEngine = {} as unknown as VerificationEngine;
		const diff = await handleKeyGenConfirmed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			keyGenClient,
			signingClient,
			CONSENSUS_STATE,
			MACHINE_STATES,
			event,
		);

		expect(diff).toStrictEqual({});
	});

	it("should only update confirmationsFrom and lastParticipant if not completed", async () => {
		const machineStates: MachineStates = {
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 20n,
				responseDeadline: 25n,
				deadline: 30n,
				missingSharesFrom: [],
				confirmationsFrom: [3n],
				complaints: {},
			},
			signing: {},
		};
		const keyGenClient = {} as unknown as KeyGenClient;
		const participants = vi.fn();
		participants.mockReturnValueOnce([1n, 2n, 3n]);
		const signingClient = {
			participants,
		} as unknown as SigningClient;
		const protocol = {} as unknown as SafenetProtocol;
		const verificationEngine = {} as unknown as VerificationEngine;
		const diff = await handleKeyGenConfirmed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			keyGenClient,
			signingClient,
			CONSENSUS_STATE,
			machineStates,
			EVENT,
		);

		expect(diff).toStrictEqual({
			rollover: {
				id: "collecting_confirmations",
				groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
				nextEpoch: 10n,
				complaintDeadline: 20n,
				responseDeadline: 25n,
				deadline: 30n,
				missingSharesFrom: [],
				confirmationsFrom: [3n, 2n],
				lastParticipant: 2n,
				complaints: {},
			},
		});
		expect(participants).toBeCalledTimes(1);
		expect(participants).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
	});

	it("should skip signing and trigger nonce tree generation if in genesis key gen", async () => {
		const keyGenClient = {} as unknown as KeyGenClient;
		const participants = vi.fn();
		participants.mockReturnValueOnce([1n, 2n, 3n]);
		const generateNonceTree = vi.fn();
		generateNonceTree.mockReturnValueOnce(keccak256("0x5afe"));
		const signingClient = {
			participants,
			generateNonceTree,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			genesisGroupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
		};
		const protocol = {} as unknown as SafenetProtocol;
		const verificationEngine = {} as unknown as VerificationEngine;
		const diff = await handleKeyGenConfirmed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			keyGenClient,
			signingClient,
			consensusState,
			MACHINE_STATES,
			EVENT,
		);

		expect(diff).toStrictEqual({
			rollover: { id: "epoch_staged", nextEpoch: 0n },
			consensus: {
				groupPendingNonces: ["0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000", true],
			},
			actions: [
				{
					id: "sign_register_nonce_commitments",
					groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
					nonceCommitmentsHash: keccak256("0x5afe"),
				},
			],
		});
		expect(participants).toBeCalledTimes(1);
		expect(participants).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(generateNonceTree).toBeCalledTimes(1);
		expect(generateNonceTree).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
	});

	it("should throw if missing public key for group", async () => {
		const groupPublicKey = vi.fn();
		groupPublicKey.mockReturnValueOnce(undefined);
		const keyGenClient = {
			groupPublicKey,
		} as unknown as KeyGenClient;
		const participants = vi.fn();
		participants.mockReturnValueOnce([1n, 2n, 3n]);
		const signingClient = {
			participants,
		} as unknown as SigningClient;
		const protocol = {} as unknown as SafenetProtocol;
		const verificationEngine = {} as unknown as VerificationEngine;
		await expect(
			handleKeyGenConfirmed(
				MACHINE_CONFIG,
				protocol,
				verificationEngine,
				keyGenClient,
				signingClient,
				CONSENSUS_STATE,
				MACHINE_STATES,
				EVENT,
			),
		).rejects.toStrictEqual(
			new Error(
				"Group public key not available for 0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			),
		);
		expect(participants).toBeCalledTimes(1);
		expect(participants).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(groupPublicKey).toBeCalledTimes(1);
		expect(groupPublicKey).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
	});

	it("should throw for invalid rollover packet", async () => {
		const groupPublicKey = vi.fn();
		groupPublicKey.mockReturnValueOnce(TEST_POINT);
		const keyGenClient = {
			groupPublicKey,
		} as unknown as KeyGenClient;
		const participants = vi.fn();
		participants.mockReturnValueOnce([1n, 2n, 3n]);
		const signingClient = {
			participants,
		} as unknown as SigningClient;
		const protocol = {
			chainId: () => 100n,
			consensus: () => "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
		} as unknown as SafenetProtocol;
		const verify = vi.fn();
		verify.mockReturnValueOnce({
			status: "invalid",
			error: "Test Error",
		});
		const verificationEngine = {
			verify,
		} as unknown as VerificationEngine;
		await expect(
			handleKeyGenConfirmed(
				MACHINE_CONFIG,
				protocol,
				verificationEngine,
				keyGenClient,
				signingClient,
				CONSENSUS_STATE,
				MACHINE_STATES,
				EVENT,
			),
		).rejects.toStrictEqual(new Error(`Invalid epoch packet created ${JSON.stringify(EPOCH_PACKET, jsonReplacer)}`));
		expect(participants).toBeCalledTimes(1);
		expect(participants).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(groupPublicKey).toBeCalledTimes(1);
		expect(groupPublicKey).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(verify).toBeCalledTimes(1);
		expect(verify).toBeCalledWith(EPOCH_PACKET);
	});

	it("should verify packet and trigger signing for rollover", async () => {
		const groupPublicKey = vi.fn();
		groupPublicKey.mockReturnValueOnce(TEST_POINT);
		const keyGenClient = {
			groupPublicKey,
		} as unknown as KeyGenClient;
		const participants = vi.fn();
		participants.mockReturnValueOnce([1n, 2n, 3n]);
		const signingClient = {
			participants,
		} as unknown as SigningClient;
		const protocol = {
			chainId: () => 100n,
			consensus: () => "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
		} as unknown as SafenetProtocol;
		const verify = vi.fn();
		verify.mockReturnValueOnce({
			status: "valid",
			packetId: keccak256("0x5afe01020304"),
		});
		const verificationEngine = {
			verify,
		} as unknown as VerificationEngine;
		const diff = await handleKeyGenConfirmed(
			MACHINE_CONFIG,
			protocol,
			verificationEngine,
			keyGenClient,
			signingClient,
			CONSENSUS_STATE,
			MACHINE_STATES,
			EVENT,
		);
		const signingState: SigningState = {
			id: "waiting_for_request",
			packet: EPOCH_PACKET,
			responsible: 2n,
			signers: [1n, 2n, 3n],
			deadline: 24n,
		};
		const rollover: RolloverState = {
			id: "sign_rollover",
			groupId: "0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000",
			nextEpoch: 10n,
			message: keccak256("0x5afe01020304"),
		};
		expect(diff).toStrictEqual({
			rollover,
			signing: [keccak256("0x5afe01020304"), signingState],
		});
		expect(participants).toBeCalledTimes(1);
		expect(participants).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(groupPublicKey).toBeCalledTimes(1);
		expect(groupPublicKey).toBeCalledWith("0x06cb03baac74421225341827941e88d9547e5459c4b3715c0000000000000000");
		expect(verify).toBeCalledTimes(1);
		expect(verify).toBeCalledWith(EPOCH_PACKET);
	});
});
