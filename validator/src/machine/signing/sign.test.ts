import { zeroHash } from "viem";
import { describe, expect, it, vi } from "vitest";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { ConsensusState, MachineConfig, MachineStates, SigningState } from "../types.js";
import { handleSign } from "./sign.js";

// --- Test Data ---
const SIGNING_STATE: SigningState = {
	id: "waiting_for_request",
	signers: [1n, 2n],
	responsible: undefined,
	deadline: 23n,
	packet: {
		type: "epoch_rollover_packet",
		domain: {
			chain: 1n,
			consensus: "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
		},
		rollover: {
			activeEpoch: 0n,
			proposedEpoch: 3n,
			rolloverBlock: 23n,
			groupKeyX: 0n,
			groupKeyY: 0n,
		},
	},
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

const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	stagedEpoch: 0n,
	groupPendingNonces: {},
	epochGroups: {},
	signatureIdToMessage: {},
};

const MACHINE_CONFIG: MachineConfig = {
	defaultParticipants: [],
	keyGenTimeout: 0n,
	signingTimeout: 20n,
	blocksPerEpoch: 0n,
};

const EVENT_ARGS = {
	initiator: "0x690f083b2968f6cB0Ab6d8885d563b7977cff43B",
	gid: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
	message: "0x5afe5afe",
	sid: "0x5af35af3",
	sequence: 0n,
};

// --- Tests ---
describe("collecting shares", () => {
	it("should fail on invalid event arguments", async () => {
		const verificationEngine = {} as unknown as VerificationEngine;
		const signingClient = {} as unknown as SigningClient;
		await expect(
			handleSign(MACHINE_CONFIG, verificationEngine, signingClient, CONSENSUS_STATE, MACHINE_STATES, 2n, {}),
		).rejects.toThrow();
	});

	it("should not handle signing shares when not collecting shares", async () => {
		const verificationEngine = {} as unknown as VerificationEngine;
		const signingClient = {} as unknown as SigningClient;
		const machineStates: MachineStates = {
			...MACHINE_STATES,
			signing: {},
		};
		const diff = await handleSign(
			MACHINE_CONFIG,
			verificationEngine,
			signingClient,
			CONSENSUS_STATE,
			machineStates,
			2n,
			EVENT_ARGS,
		);

		expect(diff).toStrictEqual({});
	});

	it("should not handle request that has not been verified", async () => {
		const isVerified = vi.fn().mockReturnValueOnce(false);
		const verificationEngine = {
			isVerified,
		} as unknown as VerificationEngine;
		const signingClient = {} as unknown as SigningClient;
		const diff = await handleSign(
			MACHINE_CONFIG,
			verificationEngine,
			signingClient,
			CONSENSUS_STATE,
			MACHINE_STATES,
			2n,
			EVENT_ARGS,
		);

		expect(isVerified).toBeCalledWith("0x5afe5afe");
		expect(isVerified).toBeCalledTimes(1);
		expect(diff).toStrictEqual({});
	});

	it("should generate nonce commitments and transition to collect nonce commitments (without new nonces)", async () => {
		const isVerified = vi.fn().mockReturnValueOnce(true);
		const verificationEngine = {
			isVerified,
		} as unknown as VerificationEngine;
		const createNonceCommitments = vi.fn().mockReturnValueOnce({
			nonceCommitments: zeroHash,
			nonceProof: [zeroHash],
		});
		const signingClient = {
			createNonceCommitments,
		} as unknown as SigningClient;
		const diff = await handleSign(
			MACHINE_CONFIG,
			verificationEngine,
			signingClient,
			CONSENSUS_STATE,
			MACHINE_STATES,
			2n,
			EVENT_ARGS,
		);

		expect(isVerified).toBeCalledWith("0x5afe5afe");
		expect(isVerified).toBeCalledTimes(1);
		expect(createNonceCommitments).toBeCalledWith(
			"0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
			"0x5af35af3",
			"0x5afe5afe",
			0n,
			[1n, 2n],
		);
		expect(createNonceCommitments).toBeCalledTimes(1);
		expect(diff.rollover).toBeUndefined();
		expect(diff.signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "collect_nonce_commitments",
				signatureId: "0x5af35af3",
				deadline: 22n,
				lastSigner: undefined,
				packet: SIGNING_STATE.packet,
			},
		]);
		expect(diff.consensus).toStrictEqual({
			signatureIdToMessage: ["0x5af35af3", "0x5afe5afe"],
		});
		expect(diff.actions).toStrictEqual([
			{
				id: "sign_reveal_nonce_commitments",
				signatureId: "0x5af35af3",
				nonceCommitments: zeroHash,
				nonceProof: [zeroHash],
			},
		]);
	});

	it("should generate nonce commitments and transition to collect nonce commitments (submit new nonces)", async () => {
		const isVerified = vi.fn().mockReturnValueOnce(true);
		const verificationEngine = {
			isVerified,
		} as unknown as VerificationEngine;
		const createNonceCommitments = vi.fn().mockReturnValueOnce({
			nonceCommitments: zeroHash,
			nonceProof: [zeroHash],
		});
		const availableNoncesCount = vi.fn().mockReturnValueOnce(10n).mockReturnValueOnce(0n);
		const generateNonceTree = vi.fn().mockReturnValueOnce(zeroHash);
		const signingClient = {
			createNonceCommitments,
			availableNoncesCount,
			generateNonceTree,
		} as unknown as SigningClient;
		const consensusState: ConsensusState = {
			...CONSENSUS_STATE,
			activeEpoch: 1n,
			epochGroups: {
				"1": {
					groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
					participantId: 1n,
				},
			},
		};
		const diff = await handleSign(
			MACHINE_CONFIG,
			verificationEngine,
			signingClient,
			consensusState,
			MACHINE_STATES,
			2n,
			EVENT_ARGS,
		);

		expect(isVerified).toBeCalledWith("0x5afe5afe");
		expect(isVerified).toBeCalledTimes(1);
		expect(createNonceCommitments).toBeCalledWith(
			"0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
			"0x5af35af3",
			"0x5afe5afe",
			0n,
			[1n, 2n],
		);
		expect(createNonceCommitments).toBeCalledTimes(1);
		expect(availableNoncesCount).toBeCalledTimes(2);
		expect(availableNoncesCount).nthCalledWith(
			1,
			"0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
			0n,
		);
		expect(availableNoncesCount).nthCalledWith(
			2,
			"0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
			1n,
		);
		expect(generateNonceTree).toBeCalledWith("0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496");
		expect(generateNonceTree).toBeCalledTimes(1);

		expect(diff.rollover).toBeUndefined();
		expect(diff.signing).toStrictEqual([
			"0x5afe5afe",
			{
				id: "collect_nonce_commitments",
				signatureId: "0x5af35af3",
				deadline: 22n,
				lastSigner: undefined,
				packet: SIGNING_STATE.packet,
			},
		]);
		expect(diff.consensus).toStrictEqual({
			groupPendingNonces: ["0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496", true],
			signatureIdToMessage: ["0x5af35af3", "0x5afe5afe"],
		});
		expect(diff.actions).toStrictEqual([
			{
				id: "sign_register_nonce_commitments",
				groupId: "0x0000000000000000000000007fa9385be102ac3eac297483dd6233d62b3e1496",
				nonceCommitmentsHash: zeroHash,
			},
			{
				id: "sign_reveal_nonce_commitments",
				signatureId: "0x5af35af3",
				nonceCommitments: zeroHash,
				nonceProof: [zeroHash],
			},
		]);
	});
});
