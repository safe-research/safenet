import Sqlite3 from "better-sqlite3";
import { type Hex, zeroAddress } from "viem";
import { beforeEach, describe, expect, it } from "vitest";
import type { EpochRolloverPacket } from "../../consensus/verify/rollover/schemas.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import type { SigningState } from "../types.js";
import { SqliteStateStorage } from "./sqlite.js";

const TX_ATTESTATION_PACKET: SafeTransactionPacket = {
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
	let testDb = new Sqlite3(":memory:");
	beforeEach(() => {
		// Reset the database for each test.
		testDb = new Sqlite3(":memory:");
	});

	it("should store and reinstantiate rollover state correctly", () => {
		const originalStorage = new SqliteStateStorage(testDb);
		expect(originalStorage.machineStates().rollover).toStrictEqual({
			id: "waiting_for_rollover",
		});
		originalStorage.applyDiff({
			rollover: {
				id: "collecting_shares",
				groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
				nextEpoch: 1n,
				deadline: 100n,
				missingSharesFrom: [1n],
				complaints: {
					"1": {
						total: 2n,
						unresponded: 1n,
					},
					"2": {
						total: 1n,
						unresponded: 0n,
					},
				},
			},
		});
		expect(originalStorage.machineStates().rollover).toStrictEqual({
			id: "collecting_shares",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			nextEpoch: 1n,
			deadline: 100n,
			missingSharesFrom: [1n],
			complaints: {
				"1": {
					total: 2n,
					unresponded: 1n,
				},
				"2": {
					total: 1n,
					unresponded: 0n,
				},
			},
		});
		const recoveredStorage = new SqliteStateStorage(testDb);
		expect(recoveredStorage.machineStates().rollover).toStrictEqual({
			id: "collecting_shares",
			groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
			nextEpoch: 1n,
			deadline: 100n,
			missingSharesFrom: [1n],
			complaints: {
				"1": {
					total: 2n,
					unresponded: 1n,
				},
				"2": {
					total: 1n,
					unresponded: 0n,
				},
			},
		});
	});

	it("should store and reinstantiate consensus state correctly", () => {
		const originalStorage = new SqliteStateStorage(testDb);
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
				epochGroup: [
					1n,
					{ groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000", participantId: 1n },
				],
				groupPendingNonces: ["0x5afe000000000000000000000000000000000000000000000000000000000000", true],
				signatureIdToMessage: [
					"0x5afe000000000000000000000000000000000000000000000000000000000000",
					"0x5af3000000000000000000000000000000000000000000000000000000000000",
				],
			},
		});
		expect(originalStorage.consensusState()).toStrictEqual({
			activeEpoch: 1n,
			stagedEpoch: 2n,
			epochGroups: {
				"1": { groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000", participantId: 1n },
			},
			groupPendingNonces: {
				"0x5afe000000000000000000000000000000000000000000000000000000000000": true,
			},
			signatureIdToMessage: {
				"0x5afe000000000000000000000000000000000000000000000000000000000000":
					"0x5af3000000000000000000000000000000000000000000000000000000000000",
			},
		});
		const recoveredStorage = new SqliteStateStorage(testDb);
		expect(recoveredStorage.consensusState()).toStrictEqual({
			activeEpoch: 1n,
			stagedEpoch: 2n,
			epochGroups: {
				"1": { groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000", participantId: 1n },
			},
			groupPendingNonces: {
				"0x5afe000000000000000000000000000000000000000000000000000000000000": true,
			},
			signatureIdToMessage: {
				"0x5afe000000000000000000000000000000000000000000000000000000000000":
					"0x5af3000000000000000000000000000000000000000000000000000000000000",
			},
		});
		// Check that cleanup is working
		recoveredStorage.applyDiff({
			consensus: {
				groupPendingNonces: ["0x5afe000000000000000000000000000000000000000000000000000000000000"],
				signatureIdToMessage: ["0x5afe000000000000000000000000000000000000000000000000000000000000"],
			},
		});
		const cleanedStorage = new SqliteStateStorage(testDb);
		expect(cleanedStorage.consensusState()).toStrictEqual({
			activeEpoch: 1n,
			stagedEpoch: 2n,
			epochGroups: {
				"1": { groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000", participantId: 1n },
			},
			groupPendingNonces: {},
			signatureIdToMessage: {},
		});
	});

	it("should store and reinstantiate signing state correctly", () => {
		const originalStorage = new SqliteStateStorage(testDb);
		expect(originalStorage.machineStates().signing).toStrictEqual({});
		// For each state one version with an epoch rollover packet and a tx attestation packet is added
		originalStorage.applyDiff({
			signing: [
				"0x5afe1a0000000000000000000000000000000000000000000000000000000000",
				{
					id: "collect_nonce_commitments",
					packet: TX_ATTESTATION_PACKET,
					signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
					lastSigner: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe1b0000000000000000000000000000000000000000000000000000000000",
				{
					id: "collect_nonce_commitments",
					packet: EPOCH_ROLLOVER_PACKET,
					signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
					lastSigner: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe2a0000000000000000000000000000000000000000000000000000000000",
				{
					id: "collect_signing_shares",
					packet: TX_ATTESTATION_PACKET,
					sharesFrom: [2n],
					signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
					lastSigner: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe2b0000000000000000000000000000000000000000000000000000000000",
				{
					id: "collect_signing_shares",
					packet: EPOCH_ROLLOVER_PACKET,
					sharesFrom: [2n],
					signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
					lastSigner: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe3a0000000000000000000000000000000000000000000000000000000000",
				{
					id: "waiting_for_attestation",
					packet: TX_ATTESTATION_PACKET,
					signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
					responsible: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe3b0000000000000000000000000000000000000000000000000000000000",
				{
					id: "waiting_for_attestation",
					packet: EPOCH_ROLLOVER_PACKET,
					signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
					responsible: 1n,
					deadline: 10n,
				},
			],
		});
		originalStorage.applyDiff({
			signing: [
				"0x5afe4a0000000000000000000000000000000000000000000000000000000000",
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
				"0x5afe4b0000000000000000000000000000000000000000000000000000000000",
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
			"0x5afe1a0000000000000000000000000000000000000000000000000000000000": {
				id: "collect_nonce_commitments",
				packet: TX_ATTESTATION_PACKET,
				signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
				lastSigner: 1n,
				deadline: 10n,
			},
			"0x5afe1b0000000000000000000000000000000000000000000000000000000000": {
				id: "collect_nonce_commitments",
				packet: EPOCH_ROLLOVER_PACKET,
				signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
				lastSigner: 1n,
				deadline: 10n,
			},
			"0x5afe2a0000000000000000000000000000000000000000000000000000000000": {
				id: "collect_signing_shares",
				packet: TX_ATTESTATION_PACKET,
				sharesFrom: [2n],
				signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
				lastSigner: 1n,
				deadline: 10n,
			},
			"0x5afe2b0000000000000000000000000000000000000000000000000000000000": {
				id: "collect_signing_shares",
				packet: EPOCH_ROLLOVER_PACKET,
				sharesFrom: [2n],
				signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
				lastSigner: 1n,
				deadline: 10n,
			},
			"0x5afe3a0000000000000000000000000000000000000000000000000000000000": {
				id: "waiting_for_attestation",
				packet: TX_ATTESTATION_PACKET,
				signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
				responsible: 1n,
				deadline: 10n,
			},
			"0x5afe3b0000000000000000000000000000000000000000000000000000000000": {
				id: "waiting_for_attestation",
				packet: EPOCH_ROLLOVER_PACKET,
				signatureId: "0x5af3010000000000000000000000000000000000000000000000000000000000",
				responsible: 1n,
				deadline: 10n,
			},
			"0x5afe4a0000000000000000000000000000000000000000000000000000000000": {
				id: "waiting_for_request",
				packet: TX_ATTESTATION_PACKET,
				signers: [1n, 2n],
				responsible: 1n,
				deadline: 10n,
			},
			"0x5afe4b0000000000000000000000000000000000000000000000000000000000": {
				id: "waiting_for_request",
				packet: EPOCH_ROLLOVER_PACKET,
				signers: [1n, 2n],
				responsible: 1n,
				deadline: 10n,
			},
		};
		expect(originalStorage.machineStates().signing).toStrictEqual(expectedSigningState);
		const recoveredStorage = new SqliteStateStorage(testDb);
		expect(recoveredStorage.machineStates().signing).toStrictEqual(expectedSigningState);
		// Delete half of the states to check that cleanup is working
		recoveredStorage.applyDiff({ signing: ["0x5afe1a0000000000000000000000000000000000000000000000000000000000"] });
		recoveredStorage.applyDiff({ signing: ["0x5afe2b0000000000000000000000000000000000000000000000000000000000"] });
		recoveredStorage.applyDiff({ signing: ["0x5afe3b0000000000000000000000000000000000000000000000000000000000"] });
		recoveredStorage.applyDiff({ signing: ["0x5afe4a0000000000000000000000000000000000000000000000000000000000"] });
		const cleanedStorage = new SqliteStateStorage(testDb);
		delete expectedSigningState["0x5afe1a0000000000000000000000000000000000000000000000000000000000"];
		delete expectedSigningState["0x5afe2b0000000000000000000000000000000000000000000000000000000000"];
		delete expectedSigningState["0x5afe3b0000000000000000000000000000000000000000000000000000000000"];
		delete expectedSigningState["0x5afe4a0000000000000000000000000000000000000000000000000000000000"];
		expect(cleanedStorage.machineStates().signing).toStrictEqual(expectedSigningState);
	});
});
