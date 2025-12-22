import type { Database } from "better-sqlite3";
import { SqliteQueue } from "../../utils/queue.js";
import { stateTransitionSchema } from "./schemas.js";
import type { StateTransition } from "./types.js";
export class SqliteTransitionQueue extends SqliteQueue<StateTransition> {
	constructor(database: Database) {
		super(stateTransitionSchema, database, "transitions");
	}
}
