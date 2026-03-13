import Sqlite3 from "better-sqlite3";
import { describe, expect, it } from "vitest";
import z from "zod";
import { InMemoryQueue, SqliteQueue } from "./queue.js";

// --- Tests ---
describe("inmemory queue", () => {
	it("should return undefined on empty pop", () => {
		const queue = new InMemoryQueue<unknown>();
		expect(queue.pop()).toBeUndefined();
	});
	it("should return last added item and undefined when empty", () => {
		const queue = new InMemoryQueue<unknown>();
		const values = [1, 2, 3, 4, 5, 6];
		for (const value of values) {
			queue.push(value);
		}
		for (const value of values) {
			expect(queue.pop()).toBe(value);
		}
		expect(queue.pop()).toBeUndefined();
	});
	it("should not delete element when peeking", () => {
		const queue = new InMemoryQueue<unknown>();
		const values = [1, 2, 3, 4, 5, 6];
		for (const value of values) {
			queue.push(value);
		}
		expect(queue.peek()).toBe(1);
		expect(queue.peek()).toBe(1);
		expect(queue.pop()).toBe(1);
		expect(queue.peek()).toBe(2);
	});
});

describe("sqlite queue", () => {
	const sqliteQueue = () => new SqliteQueue<number>(z.number(), new Sqlite3(":memory:"), "test");

	it("should return undefined on empty pop", () => {
		const queue = sqliteQueue();
		expect(queue.pop()).toBeUndefined();
	});
	it("should return last added item and undefined when empty", () => {
		const queue = sqliteQueue();
		const values = [1, 2, 3, 4, 5, 6];
		for (const value of values) {
			queue.push(value);
		}
		for (const value of values) {
			expect(queue.pop()).toBe(value);
		}
		expect(queue.pop()).toBeUndefined();
	});
	it("should not delete element when peeking", () => {
		const queue = sqliteQueue();
		const values = [1, 2, 3, 4, 5, 6];
		for (const value of values) {
			queue.push(value);
		}
		expect(queue.peek()).toBe(1);
		expect(queue.peek()).toBe(1);
		expect(queue.pop()).toBe(1);
		expect(queue.peek()).toBe(2);
	});
});
