import { readFileSync } from "node:fs";
import path from "node:path";
import Sqlite3 from "better-sqlite3";
import { entryPoint06Address } from "viem/account-abstraction";
import { describe, expect, it } from "vitest";
import { TEST_ACTIONS } from "../../__tests__/data/protocol.js";
import { SqliteActionQueue, SqliteTxStorage } from "./sqlite.js";

describe("protocol - sqlite", () => {
	describe("SqliteActionQueue", () => {
		it("should store all actions and return in correct order", () => {
			const storage = new SqliteActionQueue(new Sqlite3(":memory:"));

			expect(storage.peek()).toBeUndefined();
			for (const [action] of TEST_ACTIONS) {
				storage.push({ ...action, validUntil: 1 });
			}
			for (const [action] of TEST_ACTIONS) {
				const actionWithTimeout = { ...action, validUntil: 1 };
				expect(storage.peek()).toStrictEqual(actionWithTimeout);
				expect(storage.pop()).toStrictEqual(actionWithTimeout);
			}
			expect(storage.peek()).toBeUndefined();
		});
	});

	describe("SqliteTxStorage Migration", () => {
		it("should succesfully migrate db", () => {
			const db = new Sqlite3(":memory:");
			// Original Table
			db.exec(`
				CREATE TABLE IF NOT EXISTS transaction_storage (
					nonce INTEGER PRIMARY KEY,
					transactionJson TEXT NOT NULL,
					transactionHash TEXT DEFAULT NULL,
					createdAt DATETIME DEFAULT (unixepoch())
				);
			`);
			const migrations = ["1_add_submitted_at.sql", "2_add_fees_json.sql"];
			for (const migration of migrations) {
				const migrationPath = path.join(__dirname, "..", "..", "..", "migrations", migration);
				const migrationSql = readFileSync(migrationPath, "utf-8");
				expect(() => {
					db.exec(migrationSql);
				}).not.toThrow();
			}
			const columns = db.pragma("table_info(transaction_storage)") as Array<{ name: string }>;
			expect(columns).toStrictEqual([
				{
					cid: 0,
					dflt_value: null,
					notnull: 0,
					pk: 1,
					name: "nonce",
					type: "INTEGER",
				},
				{
					cid: 1,
					dflt_value: null,
					notnull: 1,
					pk: 0,
					name: "transactionJson",
					type: "TEXT",
				},
				{
					cid: 2,
					dflt_value: "NULL",
					notnull: 0,
					pk: 0,
					name: "transactionHash",
					type: "TEXT",
				},
				{
					cid: 3,
					dflt_value: "unixepoch()",
					notnull: 0,
					pk: 0,
					name: "createdAt",
					type: "DATETIME",
				},
				{
					cid: 4,
					dflt_value: "NULL",
					notnull: 0,
					pk: 0,
					name: "submittedAt",
					type: "INTEGER",
				},
				{
					cid: 5,
					dflt_value: "NULL",
					notnull: 0,
					pk: 0,
					name: "feesJson",
					type: "TEXT",
				},
			]);

			// Check that it works with tx storage operations
			const storage = new SqliteTxStorage(db);
			expect(storage.submittedUpTo(0n)).toStrictEqual([]);
		});

		it("should throw on migrated db", () => {
			const db = new Sqlite3(":memory:");
			// Will create db with latest schema
			new SqliteTxStorage(db);
			const migrations = ["1_add_submitted_at.sql", "2_add_fees_json.sql"];
			for (const migration of migrations) {
				const migrationPath = path.join(__dirname, "..", "..", "..", "migrations", migration);
				const migrationSql = readFileSync(migrationPath, "utf-8");
				expect(() => {
					db.exec(migrationSql);
				}).toThrow();
			}
		});
	});

	describe("SqliteTxStorage", () => {
		it("should throw in invalid stored transaction json", () => {
			const db = new Sqlite3(":memory:");
			const storage = new SqliteTxStorage(db);
			db.prepare(`
				INSERT INTO transaction_storage (nonce, transactionJson, submittedAt)
				VALUES ($nonce, $transactionJson, $submittedAt);
			`).run({
				nonce: 1,
				transactionJson: "Invalid tx JSON!",
				submittedAt: 0,
			});
			expect(() => storage.submittedUpTo(0n)).toThrow("Unexpected token 'I', \"Invalid tx JSON!\" is not valid JSON");
		});

		it("should throw in invalid fees json", () => {
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
			storage.setSubmittedForPending(0n);
			db.prepare(`
				UPDATE transaction_storage
				SET feesJson = ?
				WHERE nonce = ?;
			`).run("Invalid fees JSON!", 1);
			expect(() => storage.submittedUpTo(0n)).toThrow("Unexpected token 'I', \"Invalid fees JSON!\" is not valid JSON");
		});

		it("should return empty if nothing stored", () => {
			const storage = new SqliteTxStorage(new Sqlite3(":memory:"));
			expect(storage.submittedUpTo(0n)).toStrictEqual([]);
		});

		it("should only return entries that have are within the limit", () => {
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
			storage.setSubmittedForPending(100n);
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
				},
				2,
			);
			storage.setSubmittedForPending(400n);
			expect(storage.submittedUpTo(300n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					fees: null,
					nonce: 1,
				},
			]);
			expect(storage.submittedUpTo(4200n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					fees: null,
					nonce: 1,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					fees: null,
					nonce: 2,
				},
			]);
		});

		it("should only return specified limit of entries with submittedUpTo", () => {
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
				},
				2,
			);
			storage.setSubmittedForPending(400n);
			expect(storage.submittedUpTo(4200n, 0, 1)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					fees: null,
					nonce: 1,
				},
			]);
			expect(storage.submittedUpTo(4200n, 0, 2)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					fees: null,
					nonce: 1,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					fees: null,
					nonce: 2,
				},
			]);
		});

		it("should apply offset for submittedUpTo", () => {
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
				},
				2,
			);
			storage.setSubmittedForPending(400n);
			expect(storage.submittedUpTo(4200n, 0, 1)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					fees: null,
					nonce: 1,
				},
			]);
			expect(storage.submittedUpTo(4200n, 1, 1)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					fees: null,
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
			storage.setSubmittedForPending(0n);
			expect(storage.submittedUpTo(0n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe",
					hash: null,
					nonce: 1,
					fees: null,
				},
			]);
			storage.setHash(1, "0x5afe5afe");
			expect(storage.submittedUpTo(0n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe",
					hash: "0x5afe5afe",
					nonce: 1,
					fees: null,
				},
			]);
		});

		it("should update the transaction fees", () => {
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
			storage.setSubmittedForPending(0n);
			expect(storage.submittedUpTo(0n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe",
					hash: null,
					nonce: 1,
					fees: null,
				},
			]);
			storage.setFees(1, {
				maxFeePerGas: 102n,
				maxPriorityFeePerGas: 51n,
			});
			expect(storage.submittedUpTo(0n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe",
					hash: null,
					nonce: 1,
					fees: {
						maxFeePerGas: 102n,
						maxPriorityFeePerGas: 51n,
					},
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
			storage.setSubmittedForPending(0n);
			expect(storage.submittedUpTo(0n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					nonce: 1,
					fees: null,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe03",
					hash: null,
					nonce: 3,
					fees: null,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					nonce: 4,
					fees: null,
				},
			]);
		});

		it("should not return deleted transactions", () => {
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
			storage.setSubmittedForPending(0n);
			expect(storage.submittedUpTo(0n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					nonce: 1,
					fees: null,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					gas: 200_000n,
					nonce: 2,
					fees: null,
				},
			]);
			storage.delete(1);
			expect(storage.submittedUpTo(0n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					gas: 200_000n,
					nonce: 2,
					fees: null,
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
			storage.setSubmittedForPending(0n);
			expect(storage.submittedUpTo(0n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe01",
					hash: null,
					nonce: 1,
					fees: null,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					gas: 200_000n,
					nonce: 2,
					fees: null,
				},
			]);
			storage.setExecuted(1);
			expect(storage.submittedUpTo(0n)).toStrictEqual([
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					gas: 200_000n,
					nonce: 2,
					fees: null,
				},
			]);
		});

		it("should return null for maxNonce if no transations stored", () => {
			const db = new Sqlite3(":memory:");
			const storage = new SqliteTxStorage(db);
			expect(storage.maxNonce()).toBe(null);
		});

		it("should return correct maxNonce", () => {
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
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe03",
					gas: 200_000n,
				},
				1,
			);
			expect(storage.maxNonce()).toBe(3);
		});

		it("should return correct number of updated executed transaction", () => {
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
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe03",
					gas: 200_000n,
				},
				1,
			);
			expect(storage.setAllBeforeAsExecuted(3)).toBe(2);
		});

		it("should return correct number of updated pending transaction", () => {
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
			expect(storage.setSubmittedForPending(0n)).toBe(2);
			storage.register(
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe03",
					gas: 200_000n,
				},
				1,
			);
			expect(storage.setSubmittedForPending(0n)).toBe(1);
		});
	});
});
