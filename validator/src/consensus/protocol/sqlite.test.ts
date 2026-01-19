import Sqlite3 from "better-sqlite3";
import { entryPoint06Address } from "viem/account-abstraction";
import { describe, expect, it } from "vitest";
import { TEST_ACTIONS } from "../../__tests__/data/protocol.js";
import { jsonReplacer } from "../../utils/json.js";
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
					data: "0x5afe03",
					hash: null,
					nonce: 3,
				},
				{
					to: entryPoint06Address,
					value: 0n,
					data: "0x5afe02",
					hash: null,
					nonce: 4,
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

		it("should return correct number of updated transaction", () => {
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
	});
});
