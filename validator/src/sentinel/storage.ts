import type { Database, Statement } from "better-sqlite3";
import type { Hex } from "viem";
import { z } from "zod";
import { hexBytes32Schema } from "../types/schemas.js";
import { jsonReplacer } from "../utils/json.js";
import type { SentinelAction, SentinelRequestState, SentinelStateDiff } from "./types.js";

const requestQueryResultSchema = z.array(
	z.object({
		id: hexBytes32Schema,
		stateJson: z.string(),
	}),
);

const sentinelRequestStateSchema = z.object({
	deadline: z.coerce.bigint().nonnegative(),
	status: z.enum(["preparing", "pending", "committed", "finalized"]),
	approve: z.boolean(),
});

export class SentinelStateStorage {
	#db: Database;
	#requests: Record<Hex, SentinelRequestState>;
	#insertStmt: Statement;
	#deleteStmt: Statement;

	constructor(database: Database) {
		this.#db = database;
		this.#db.exec(`
			CREATE TABLE IF NOT EXISTS sentinel_requests (
				id TEXT PRIMARY KEY,
				stateJson TEXT NOT NULL
			);
		`);
		this.#insertStmt = this.#db.prepare(`
			INSERT INTO sentinel_requests (id, stateJson)
			VALUES (?, ?)
			ON CONFLICT(id) DO UPDATE SET
				stateJson = excluded.stateJson
		`);
		this.#deleteStmt = this.#db.prepare("DELETE FROM sentinel_requests WHERE id = ?");
		this.#requests = this.#load();
	}

	#load(): Record<Hex, SentinelRequestState> {
		const rows = requestQueryResultSchema.parse(this.#db.prepare("SELECT id, stateJson FROM sentinel_requests").all());
		const requests: Record<Hex, SentinelRequestState> = {};
		for (const row of rows) {
			const data = JSON.parse(row.stateJson);
			requests[row.id] = sentinelRequestStateSchema.parse(data);
		}
		return requests;
	}

	requests(): Readonly<Record<Hex, SentinelRequestState>> {
		return this.#requests;
	}

	applyDiff(diff: SentinelStateDiff): SentinelAction[] {
		if (diff.request) {
			const [id, state] = diff.request;
			if (state === undefined) {
				this.#deleteStmt.run(id);
				delete this.#requests[id];
			} else {
				const stateJson = JSON.stringify(state, jsonReplacer);
				this.#insertStmt.run(id, stateJson);
				this.#requests[id] = state;
			}
		}
		return diff.actions ?? [];
	}
}
