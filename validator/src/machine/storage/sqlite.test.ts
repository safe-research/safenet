import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import type { Hex } from "viem";
import { afterAll, beforeEach, describe, expect, it } from "vitest";
import type { EpochRolloverPacket } from "../../consensus/verify/rollover/schemas.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import type { SigningState } from "../types.js";
import { SqliteStateStorage } from "./sqlite.js";

// Generate a unique file name using a high-resolution timestamp to prevent parallel test conflicts
const TEST_DB_FILENAME = `test-storage-${process.pid}-${Date.now()}.sqlite`;
const TEST_DB_PATH = path.join(os.tmpdir(), TEST_DB_FILENAME);

const TX_ATTESTATION_PACKET: SafeTransactionPacket = {
	type: "safe_transaction_packet",
	domain: {
		chain: 1n,
		consensus: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
	},
	proposal: {
		epoch: 22n,
		transaction: {
			to: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
			value: 0n,
			data: "0x",
			operation: 0,
			nonce: 0n,
			chainId: 0n,
			account: "0x89bEf0f3a116cf717e51F74C271A0a7aF527511D",
		},
	},
};

const EPOCH_ROLLOVER_PACKET: EpochRolloverPacket = {
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
};

describe("SqliteStateStorage", () => {
	beforeEach(() => {
		// Ensure that there is not left over from the test before
		if (fs.existsSync(TEST_DB_PATH)) {
			fs.unlinkSync(TEST_DB_PATH);
		}
	});

	afterAll(() => {
		// Clean up after all test runs
		if (fs.existsSync(TEST_DB_PATH)) {
			fs.unlinkSync(TEST_DB_PATH);
		}
	});

	it("should store and reinstantiate rollover state correctly", () => {
		const originalStorage = new SqliteStateStorage(TEST_DB_PATH);
		expect(originalStorage.machineStates().rollover).toStrictEqual({
			id: "waiting_for_rollover",
		});
		originalStorage.applyDiff({
			rollover: {
				id: "collecting_shares",
				groupId: "0x5afe",
				nextEpoch: 1n,
				deadline: 100n,
			},
		});
		expect(originalStorage.machineStates().rollover).toStrictEqual({
			id: "collecting_shares",
			groupId: "0x5afe",
			nextEpoch: 1n,
			deadline: 100n,
		});
		const recoveredStorage = new SqliteStateStorage(TEST_DB_PATH);
		expect(recoveredStorage.machineStates().rollover).toStrictEqual({
			id: "collecting_shares",
			groupId: "0x5afe",
			nextEpoch: 1n,
			deadline: 100n,
		});
	});

	it("should store and reinstantiate consensus state correctly", () => {
		const originalStorage = new SqliteStateStorage(TEST_DB_PATH);
		expect(originalStorage.consensusState()).toStrictEqual({
			activeEpoch: 0n,
			stagedEpoch: 0n,
			epochGroups: {},
			groupPendingNonces: {},
			signatureIdToMessage: {},
		});
		originalStorage.applyDiff({
			consensus: {
				activeEpoch: 1n,
				stagedEpoch: 2n,
				epochGroup: [1n, { groupId: "0x5afe", participantId: 1n }],
				groupPendingNonces: ["0x5afe", true],
				signatureIdToMessage: ["0x5afe", "0x5af3"],
			},
		});
		expect(originalStorage.consensusState()).toStrictEqual({
			activeEpoch: 1n,
			stagedEpoch: 2n,
			epochGroups: {
				"1": { groupId: "0x5afe", participantId: 1n },
			},
			groupPendingNonces: {
				"0x5afe": true,
			},
			signatureIdToMessage: {
				"0x5afe": "0x5af3",
			},
		});
		const recoveredStorage = new SqliteStateStorage(TEST_DB_PATH);
		expect(recoveredStorage.consensusState()).toStrictEqual({
			activeEpoch: 1n,
			stagedEpoch: 2n,
			epochGroups: {
				"1": { groupId: "0x5afe", participantId: 1n },
			},
			groupPendingNonces: {
				"0x5afe": true,
			},
			signatureIdToMessage: {
				"0x5afe": "0x5af3",
			},
		});
		// Check that cleanup is working
		recoveredStorage.applyDiff({
			consensus: {
				groupPendingNonces: ["0x5afe"],
				signatureIdToMessage: ["0x5afe"],
			},
		});
		const cleanedStorage = new SqliteStateStorage(TEST_DB_PATH);
		expect(cleanedStorage.consensusState()).toStrictEqual({
			activeEpoch: 1n,
			stagedEpoch: 2n,
			epochGroups: {
				"1": { groupId: "0x5afe", participantId: 1n },
			},
			groupPendingNonces: {},
			signatureIdToMessage: {},
		});
	});

	it("should store and reinstantiate signing state correctly", () => {
		const originalStorage = new SqliteStateStorage(TEST_DB_PATH);
		expect(originalStorage.machineStates().signing).toStrictEqual({});
		// For each state one version with an epoch rollover packet and a tx attestation packet is added
		originalStorage.applyDiff({
			signing: [
				"0x5afe1a",
				{
					id: "collect_nonce_commitments",
					packet: TX_ATTESTATION_PACKET,
					signatureId: "0x5af301",
					lastSigner: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe1b",
				{
					id: "collect_nonce_commitments",
					packet: EPOCH_ROLLOVER_PACKET,
					signatureId: "0x5af301",
					lastSigner: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe2a",
				{
					id: "collect_signing_shares",
					packet: TX_ATTESTATION_PACKET,
					sharesFrom: [2n],
					signatureId: "0x5af301",
					lastSigner: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe2b",
				{
					id: "collect_signing_shares",
					packet: EPOCH_ROLLOVER_PACKET,
					sharesFrom: [2n],
					signatureId: "0x5af301",
					lastSigner: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe3a",
				{
					id: "waiting_for_attestation",
					packet: TX_ATTESTATION_PACKET,
					signatureId: "0x5af301",
					responsible: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe3b",
				{
					id: "waiting_for_attestation",
					packet: EPOCH_ROLLOVER_PACKET,
					signatureId: "0x5af301",
					responsible: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe4a",
				{
					id: "waiting_for_request",
					packet: TX_ATTESTATION_PACKET,
					signers: [1n, 2n],
					responsible: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe4b",
				{
					id: "waiting_for_request",
					packet: EPOCH_ROLLOVER_PACKET,
					signers: [1n, 2n],
					responsible: 1n,
					deadline: 10n,
				},
			],
		});
		const expectedSigningState: Record<Hex, SigningState> = {
			"0x5afe1a": {
				id: "collect_nonce_commitments",
				packet: TX_ATTESTATION_PACKET,
				signatureId: "0x5af301",
				lastSigner: 1n,
				deadline: 10n,
			},
			"0x5afe1b": {
				id: "collect_nonce_commitments",
				packet: EPOCH_ROLLOVER_PACKET,
				signatureId: "0x5af301",
				lastSigner: 1n,
				deadline: 10n,
			},
			"0x5afe2a": {
				id: "collect_signing_shares",
				packet: TX_ATTESTATION_PACKET,
				sharesFrom: [2n],
				signatureId: "0x5af301",
				lastSigner: 1n,
				deadline: 10n,
			},
			"0x5afe2b": {
				id: "collect_signing_shares",
				packet: EPOCH_ROLLOVER_PACKET,
				sharesFrom: [2n],
				signatureId: "0x5af301",
				lastSigner: 1n,
				deadline: 10n,
			},
			"0x5afe3a": {
				id: "waiting_for_attestation",
				packet: TX_ATTESTATION_PACKET,
				signatureId: "0x5af301",
				responsible: 1n,
				deadline: 10n,
			},
			"0x5afe3b": {
				id: "waiting_for_attestation",
				packet: EPOCH_ROLLOVER_PACKET,
				signatureId: "0x5af301",
				responsible: 1n,
				deadline: 10n,
			},
			"0x5afe4a": {
				id: "waiting_for_request",
				packet: TX_ATTESTATION_PACKET,
				signers: [1n, 2n],
				responsible: 1n,
				deadline: 10n,
			},
			"0x5afe4b": {
				id: "waiting_for_request",
				packet: EPOCH_ROLLOVER_PACKET,
				signers: [1n, 2n],
				responsible: 1n,
				deadline: 10n,
			},
		};
		expect(originalStorage.machineStates().signing).toStrictEqual(expectedSigningState);
		const recoveredStorage = new SqliteStateStorage(TEST_DB_PATH);
		expect(recoveredStorage.machineStates().signing).toStrictEqual(expectedSigningState);
		// Delete half of the states to check that cleanup is working
		recoveredStorage.applyDiff({ signing: ["0x5afe1a"] });
		recoveredStorage.applyDiff({ signing: ["0x5afe2b"] });
		recoveredStorage.applyDiff({ signing: ["0x5afe3b"] });
		recoveredStorage.applyDiff({ signing: ["0x5afe4a"] });
		const cleanedStorage = new SqliteStateStorage(TEST_DB_PATH);
		delete expectedSigningState["0x5afe1a"];
		delete expectedSigningState["0x5afe2b"];
		delete expectedSigningState["0x5afe3b"];
		delete expectedSigningState["0x5afe4a"];
		expect(cleanedStorage.machineStates().signing).toStrictEqual(expectedSigningState);
	});
});
