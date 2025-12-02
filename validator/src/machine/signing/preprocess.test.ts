import { describe, expect, it, vi } from "vitest";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { ConsensusState } from "../types.js";
import { handlePreprocess } from "./preprocess.js";

// --- Test Data ---
const CONSENSUS_STATE: ConsensusState = {
	activeEpoch: 0n,
	stagedEpoch: 0n,
	groupPendingNonces: {
		"0x000000000000000000000000000000000000000000000000000000005af35af3": true,
	},
	epochGroups: {},
	signatureIdToMessage: {},
};
const EVENT_ARGS = {
	gid: "0x000000000000000000000000000000000000000000000000000000005af35af3",
	identifier: 1n,
	chunk: 0n,
	commitment: "0x5af35af35af35af35af35af35af35af35af35af35af35af35af35af35af35af3",
};

// --- Tests ---
describe("handle preprocess", () => {
	it("should fail on invalid event arguments", async () => {
		const signingClient = {} as unknown as SigningClient;
		await expect(handlePreprocess(signingClient, CONSENSUS_STATE, {})).rejects.toThrow();
	});

	it("should remove group from pending nonces", async () => {
		const handleNonceCommitmentsHash = vi.fn();
		const signingClient = {
			handleNonceCommitmentsHash,
		} as unknown as SigningClient;
		const diff = await handlePreprocess(signingClient, CONSENSUS_STATE, EVENT_ARGS);

		expect(handleNonceCommitmentsHash).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			1n,
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
		const diff = await handlePreprocess(signingClient, consensusState, EVENT_ARGS);

		expect(handleNonceCommitmentsHash).toBeCalledWith(
			"0x000000000000000000000000000000000000000000000000000000005af35af3",
			1n,
			"0x5af35af35af35af35af35af35af35af35af35af35af35af35af35af35af35af3",
			0n,
		);
		expect(handleNonceCommitmentsHash).toBeCalledTimes(1);

		expect(diff.signing).toBeUndefined();
		expect(diff.rollover).toBeUndefined();
		expect(diff.actions).toBeUndefined();
		expect(diff.consensus).toStrictEqual({});
	});
});
