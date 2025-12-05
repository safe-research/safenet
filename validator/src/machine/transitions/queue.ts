import { SqliteQueue } from "../../utils/queue.js";
import { stateTransitionSchema } from "./schemas.js";
import type { StateTransition } from "./types.js";
export class SqliteTransitionQueue extends SqliteQueue<StateTransition> {
	constructor(path: string) {
		super(stateTransitionSchema, path, "transitions");
	}
}
