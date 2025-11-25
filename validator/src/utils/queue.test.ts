import { describe, expect, it } from "vitest";
import { Queue } from "./queue.js";

// --- Tests ---
describe("queue", () => {
	it("should return undefined on empty pop", () => {
		const queue = new Queue<unknown>();
		expect(queue.pop()).toBeUndefined();
	});
	it("should return last added item and undefined when empty", () => {
		const queue = new Queue<unknown>();
		const values = [1, 2, 3, 4, 5, 6];
		for (const value of values) {
			queue.push(value);
		}
		for (const value of values) {
			expect(queue.pop()).toBe(value);
		}
		expect(queue.pop()).toBeUndefined();
	});
	it("should add element at the end when using return", () => {
		const queue = new Queue<unknown>();
		const values = [1, 2, 3, 4, 5, 6];
		for (const value of values) {
			queue.push(value);
		}
		queue.return(7);
		expect(queue.pop()).toBe(7);
		expect(queue.pop()).toBe(1);
	});
});
