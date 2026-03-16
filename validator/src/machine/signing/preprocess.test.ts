import { ethAddress, zeroHash } from "viem";
import { describe, expect, it, vi } from "vitest";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { NonceCommitmentsHashEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig } from "../types.js";
import { handlePreprocess } from "./preprocess.js";

// --- Test Data ---
const MACHINE_CONFIG: MachineConfig = {
	account: "0x0000000000000000000000000000000000005aFE",
	participantsInfo: [],
	genesisSalt: zeroHash,
	keyGenTimeout: 0n,
	signingTimeout: 20n,
	blocksPerEpoch: 8n,
};
const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	groupPendingNonces: {
		"0x000000000000000000000000000000000000000000000000000000005af35af3": true,
	},
	epochGroups: {},
	signatureIdToMessage: {},
};
const EVENT: NonceCommitmentsHashEvent = {
	id: "event_nonce_commitments_hash",
	block: 2n,
	index: 0,
	gid: "0x000000000000000000000000000000000000000000000000000000005af35af3",
	participant: "0x0000000000000000000000000000000000005aFE",
	chunk: 0n,
	commitment: "0x5af35af35af35af35af35af35af35af35af35af35af35af35af35af35af35af3",
};

// --- Tests ---
describe("handle preprocess", () => {
	it("should remove group from pending nonces", async () => {
		const handleNonceCommitmentsHash = vi.fn();
		const signingClient = {
			handleNonceCommitmentsHash,
		} as unknown as SigningClient;
		const diff = await handlePreprocess(MACHINE_CONFIG, signingClient, CONSENSUS_STATE, EVENT);

		expect(handleNonceCommitmentsHash).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			"0x5af35af35af35af35af35af35af35af35af35af35af35af35af35af35af35af3",
			0n,
		);
		expect(handleNonceCommitmentsHash).toBeCalledTimes(1);

		expect(diff.signing).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.actions).toBeUndefined();
		expect(diff.consensus).toStrictEqual({
			groupPendingNonces: ["0x000000000000000000000000000000000000000000000000000000005af35af3"],
		});
	});

	it("should handle nonces for untracked group", async () => {
		const handleNonceCommitmentsHash = vi.fn();
		const signingClient = {
			handleNonceCommitmentsHash,
		} as unknown as SigningClient;
		const consensusState = {
			...CONSENSUS_STATE,
			groupPendingNonces: {},
		};
		const diff = await handlePreprocess(MACHINE_CONFIG, signingClient, consensusState, EVENT);

		expect(handleNonceCommitmentsHash).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			"0x5af35af35af35af35af35af35af35af35af35af35af35af35af35af35af35af3",
			0n,
		);
		expect(handleNonceCommitmentsHash).toBeCalledTimes(1);

		expect(diff.signing).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.actions).toBeUndefined();
		expect(diff.consensus).toStrictEqual({});
	});

	it("should not handle nonces of other accounts", async () => {
		const handleNonceCommitmentsHash = vi.fn();
		const signingClient = {
			handleNonceCommitmentsHash,
		} as unknown as SigningClient;
		const machineConfig = {
			...MACHINE_CONFIG,
			account: ethAddress,
		};
		const consensusState = {
			...CONSENSUS_STATE,
			groupPendingNonces: {},
		};
		const diff = await handlePreprocess(machineConfig, signingClient, consensusState, EVENT);

		expect(handleNonceCommitmentsHash).toBeCalledTimes(0);

		expect(diff.signing).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.actions).toBeUndefined();
		expect(diff.consensus).toStrictEqual({});
	});
});
