import Sqlite3 from "better-sqlite3";
import { describe, expect, it } from "vitest";
import z from "zod";
import { SqliteQueue } from "./queue.js";

describe("sqlite queue", () => {
	const sqliteQueue = () => new SqliteQueue<number>(z.number(), new Sqlite3(":memory:"), "test");

	it("should return undefined on empty pop", () => {
		const queue = sqliteQueue();
		expect(queue.dequeue()).toBeUndefined();
	});
	it("should return last added item and undefined when empty", () => {
		const queue = sqliteQueue();
		const values = [1, 2, 3, 4, 5, 6];
		for (const value of values) {
			queue.enqueue(value);
		}
		for (const value of values) {
			expect(queue.dequeue()).toBe(value);
		}
		expect(queue.dequeue()).toBeUndefined();
	});
	it("should not delete element when peeking", () => {
		const queue = sqliteQueue();
		const values = [1, 2, 3, 4, 5, 6];
		for (const value of values) {
			queue.enqueue(value);
		}
		expect(queue.peek()).toBe(1);
		expect(queue.peek()).toBe(1);
		expect(queue.dequeue()).toBe(1);
		expect(queue.peek()).toBe(2);
	});
});
