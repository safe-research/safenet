import Sqlite3 from "better-sqlite3";
import { describe, expect, it } from "vitest";
import { TEST_EVENTS } from "../../__tests__/data/protocol.js";
import { SqliteTransitionQueue } from "./queue.js";

describe("SqliteActionQueue", () => {
	it("should store all actions and return in correct order", () => {
		const storage = new SqliteTransitionQueue(new Sqlite3(":memory:"));

		expect(storage.peek()).toBeUndefined();
		for (const [, action] of TEST_EVENTS) {
			storage.push(action);
		}
		for (const [, action] of TEST_EVENTS) {
			expect(storage.peek()).toStrictEqual(action);
			expect(storage.pop()).toStrictEqual(action);
		}
		expect(storage.peek()).toBeUndefined();
	});
});
