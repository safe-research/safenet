import type { Database } from "better-sqlite3";
import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { SignatureId } from "../../frost/types.js";
import { jsonReplacer } from "../../utils/json.js";
import type { ConsensusState, MutableConsensusState, RolloverState, SigningState, StateDiff } from "../types.js";
import { InMemoryStateStorage } from "./inmemory.js";
import {
	consensusStateSchema,
	jsonQueryResultSchema,
	rolloverStateSchema,
	signingQueryResultSchema,
	signingStateSchema,
} from "./schemas.js";

function loadConsensusState(db: Database): MutableConsensusState | undefined {
	const stmt = db.prepare("SELECT stateJson FROM consensus_state WHERE id = 1");
	const result = jsonQueryResultSchema.parse(stmt.get());

	if (!result) {
		// No entries stored, lets start fresh
		return undefined;
	}

	// If this fails we should abort as the db is in an invalid state
	const data = JSON.parse(result.stateJson);
	return consensusStateSchema.parse(data);
}

function writeConsensusState(db: Database, state: ConsensusState): void {
	const stateJson = JSON.stringify(state, jsonReplacer);
	db.prepare(`
		INSERT INTO consensus_state (id, stateJson)
		VALUES (1, ?)
		ON CONFLICT(id) DO UPDATE SET
			stateJson = excluded.stateJson;
	`).run(stateJson);
}

function loadSigningStates(db: Database): Record<SignatureId, SigningState> {
	const stmt = db.prepare("SELECT signatureId, stateJson FROM signing_states");
	const results = signingQueryResultSchema.parse(stmt.all());

	const signingStates: Record<SignatureId, SigningState> = {};

	for (const row of results) {
		// If this fails we should abort as the db is in an invalid state
		const data = JSON.parse(row.stateJson);
		signingStates[row.signatureId] = signingStateSchema.parse(data);
	}

	return signingStates;
}

function deleteSigningState(db: Database, id: SignatureId): void {
	db.prepare(`
		DELETE FROM signing_states
		WHERE signatureId = ?;
	`).run(id);
}

function writeSigningState(db: Database, id: SignatureId, state: SigningState): void {
	const stateJson = JSON.stringify(state, jsonReplacer);
	db.prepare(`
		INSERT INTO signing_states (signatureId, stateJson)
		VALUES (?, ?)
		ON CONFLICT(signatureId) DO UPDATE SET
			stateJson = excluded.stateJson;
	`).run(id, stateJson);
}

function loadRolloverState(db: Database): RolloverState | undefined {
	const stmt = db.prepare("SELECT stateJson FROM rollover_state WHERE id = 1");
	const result = jsonQueryResultSchema.parse(stmt.get());

	if (!result) {
		// No entries stored, lets start fresh
		return undefined;
	}

	// If this fails we should abort as the db is in an invalid state
	const data = JSON.parse(result.stateJson);
	return rolloverStateSchema.parse(data);
}

function writeRolloverState(db: Database, state: RolloverState): void {
	const stateJson = JSON.stringify(state, jsonReplacer);
	db.prepare(`
		INSERT INTO rollover_state (id, stateJson)
		VALUES (1, ?)
		ON CONFLICT(id) DO UPDATE SET
			stateJson = excluded.stateJson;
	`).run(stateJson);
}

export class SqliteStateStorage extends InMemoryStateStorage {
	#db: Database;

	constructor(database: Database) {
		database.exec(`
			CREATE TABLE IF NOT EXISTS consensus_state (
				-- Enforce a single row for the global consensus data
				id INTEGER PRIMARY KEY CHECK (id = 1),

				-- Stores the JSON serialized representation of MutableConsensusState
				stateJson TEXT NOT NULL
			);
			CREATE TABLE IF NOT EXISTS rollover_state (
				-- Enforce a single row for the active rollover process
				id INTEGER PRIMARY KEY CHECK (id = 1),

				-- Stores the JSON serialized representation of RolloverState
				stateJson TEXT NOT NULL
			);
			CREATE TABLE IF NOT EXISTS signing_states (
				-- The SignatureId is the unique key for each signing session
				signatureId TEXT PRIMARY KEY NOT NULL,

				-- Stores the JSON serialized representation of a single SigningState object
				stateJson TEXT NOT NULL
			);
		`);

		// Load the database state and initialize the InMemoryStateStorage with it
		const consensus = loadConsensusState(database);
		const rollover = loadRolloverState(database);
		const signing = loadSigningStates(database);
		super(consensus, { rollover, signing });

		this.#db = database;
	}

	applyDiff(diff: StateDiff): ProtocolAction[] {
		// Use the actions from the inmemory storage as a return value
		const actions = super.applyDiff(diff);
		// Sync the db with the inmemory storage
		this.#db.transaction(() => {
			writeConsensusState(this.#db, super.consensusState());
			if (diff.rollover) {
				writeRolloverState(this.#db, diff.rollover);
			}
			if (diff.signing) {
				const [signatureId, update] = diff.signing;
				if (update === undefined) {
					deleteSigningState(this.#db, signatureId);
				} else {
					writeSigningState(this.#db, signatureId, update);
				}
			}
		})();
		return actions;
	}
}
