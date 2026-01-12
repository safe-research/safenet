import Sqlite3 from "better-sqlite3";
import { entryPoint06Address } from "viem/account-abstraction";
import { describe, expect, it } from "vitest";
import { toPoint } from "../../frost/math.js";
import { jsonReplacer } from "../../utils/json.js";
import { SqliteActionQueue, SqliteTxStorage } from "./sqlite.js";
import type { ActionWithTimeout } from "./types.js";

const TEST_POINT = toPoint({
	x: 105587021125387004117772930966558154492652686110919450580386247155506502192059n,
	y: 97790146336079427917878178932139533907352200097479391118658154349645214584696n,
});

const actions: ActionWithTimeout[] = [
	{
		id: "sign_request",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		message: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
		validUntil: 0,
	},
	{
		id: "sign_register_nonce_commitments",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		nonceCommitmentsHash: "0x5afe5afe00000000000000000000000000000000000000000000000000000000",
		validUntil: 0,
	},
	{
		id: "sign_reveal_nonce_commitments",
		signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		nonceCommitments: {
			bindingNonceCommitment: TEST_POINT,
			hidingNonceCommitment: TEST_POINT,
		},
		nonceProof: [
			"0x5afe010000000000000000000000000000000000000000000000000000000000",
			"0x5afe020000000000000000000000000000000000000000000000000000000000",
		],
		validUntil: 1,
	},
	{
		id: "sign_publish_signature_share",
		signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		signersRoot: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		signersProof: [
			"0x5afe010000000000000000000000000000000000000000000000000000000000",
			"0x5afe020000000000000000000000000000000000000000000000000000000000",
		],
		groupCommitment: TEST_POINT,
		commitmentShare: TEST_POINT,
		signatureShare: 1n,
		lagrangeCoefficient: 2n,
		validUntil: 1,
	},
	{
		id: "sign_publish_signature_share",
		signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		signersRoot: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		signersProof: [
			"0x5afe010000000000000000000000000000000000000000000000000000000000",
			"0x5afe020000000000000000000000000000000000000000000000000000000000",
		],
		groupCommitment: TEST_POINT,
		commitmentShare: TEST_POINT,
		signatureShare: 1n,
		lagrangeCoefficient: 2n,
		callbackContext: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		validUntil: 1,
	},
	{
		id: "key_gen_start",
		participants: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		count: 4,
		threshold: 3,
		context: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		participantId: 1n,
		commitments: [TEST_POINT, TEST_POINT],
		pok: {
			r: TEST_POINT,
			mu: 5n,
		},
		poap: [
			"0x5afe010000000000000000000000000000000000000000000000000000000000",
			"0x5afe020000000000000000000000000000000000000000000000000000000000",
		],
		validUntil: 1,
	},
	{
		id: "key_gen_publish_secret_shares",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		verificationShare: TEST_POINT,
		shares: [1n, 2n, 3n, 5n, 8n, 13n],
		validUntil: 1,
	},
	{
		id: "key_gen_complain",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		accused: 1n,
		validUntil: 1,
	},
	{
		id: "key_gen_complaint_response",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		plaintiff: 2n,
		secretShare: 0x5afe5afe5afen,
		validUntil: 1,
	},
	{
		id: "key_gen_confirm",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		validUntil: 1,
	},
	{
		id: "key_gen_confirm",
		groupId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		callbackContext: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		validUntil: 1,
	},
	{
		id: "consensus_attest_transaction",
		epoch: 10n,
		transactionHash: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		validUntil: 1,
	},
	{
		id: "consensus_stage_epoch",
		proposedEpoch: 10n,
		rolloverBlock: 30n,
		groupId: "0x5afe00aa00000000000000000000000000000000000000000000000000000000",
		signatureId: "0x5afe000000000000000000000000000000000000000000000000000000000000",
		validUntil: 1,
	},
];

describe("protocol - sqlite", () => {
	describe("SqliteActionQueue", () => {
		it("should store all actions and return in correct order", () => {
			const storage = new SqliteActionQueue(new Sqlite3(":memory:"));

			expect(storage.peek()).toBeUndefined();
			for (const action of actions) {
				storage.push(action);
			}
			for (const action of actions) {
				expect(storage.peek()).toStrictEqual(action);
				expect(storage.pop()).toStrictEqual(action);
			}
			expect(storage.peek()).toBeUndefined();
		});
	});

	describe("SqliteTxStorage", () => {
		it("should throw in invalid stored json", () => {
			const db = new Sqlite3(":memory:");
			const storage = new SqliteTxStorage(db);
			db.prepare(`
				INSERT INTO transaction_storage (nonce, transactionJson)
				VALUES ($nonce, $transactionJson);
			`).run({
				nonce: 1,
				transactionJson: "Invalid JSON!",
			});
			const _pending = db
				.prepare(`
				SELECT * FROM transaction_storage;
			`)
				.all();

			expect(() => storage.pending(0)).toThrow("Unexpected token 'I', \"Invalid JSON!\" is not valid JSON");
		});

		it("should return empty if nothing stored", () => {
			const storage = new SqliteTxStorage(new Sqlite3(":memory:"));
			expect(storage.pending(0)).toStrictEqual([]);
		});

		it("should only return entries that have are within the limit", () => {
			const db = new Sqlite3(":memory:");
			const storage = new SqliteTxStorage(db);
			// Insert before the time
			db.prepare(`
				INSERT INTO transaction_storage (nonce, transactionJson, createdAt)
				VALUES ($nonce, $transactionJson, $createdAt);
			`).run({
				nonce: 1,
				transactionJson: JSON.stringify(
					{
						to: entryPoint06Address,
						value: 0n,
						data: "0x",
					},
					jsonReplacer,
				),
				createdAt: Date.now() / 1000 - 600,
			});
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe",
				},
				1,
			);
			expect(storage.pending(300)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x",
					hash: null,
					nonce: 1,
				},
			]);
			expect(storage.pending(0)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x",
					hash: null,
					nonce: 1,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe",
					hash: null,
					nonce: 2,
				},
			]);
		});

		it("should update the transaction hash", () => {
			const db = new Sqlite3(":memory:");
			const storage = new SqliteTxStorage(db);
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe",
				},
				1,
			);
			expect(storage.pending(0)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe",
					hash: null,
					nonce: 1,
				},
			]);
			storage.setHash(1, "0x5afe5afe");
			expect(storage.pending(0)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe",
					hash: "0x5afe5afe",
					nonce: 1,
				},
			]);
		});

		it("should use min nonce if free (instead of highest nonce)", () => {
			const db = new Sqlite3(":memory:");
			const storage = new SqliteTxStorage(db);
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
				},
				1,
			);
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe03",
				},
				3,
			);
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
				},
				2,
			);
			expect(storage.pending(0)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					nonce: 1,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					nonce: 2,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe03",
					hash: null,
					nonce: 3,
				},
			]);
		});

		it("should not return executed transactions", () => {
			const db = new Sqlite3(":memory:");
			const storage = new SqliteTxStorage(db);
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
				},
				1,
			);
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					gas: 200_000n,
				},
				1,
			);
			expect(storage.pending(0)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					nonce: 1,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					gas: 200_000n,
					nonce: 2,
				},
			]);
			storage.setExecuted(1);
			expect(storage.pending(0)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					gas: 200_000n,
					nonce: 2,
				},
			]);
		});
	});
});
