import Sqlite3, { type Database } from "better-sqlite3";
import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { SignatureId } from "../../frost/types.js";
import type { ConsensusState, MutableConsensusState, RolloverState, SigningState, StateDiff } from "../types.js";
import { InMemoryStateStorage } from "./inmemory.js";
import {
	consensusStateSchema,
	jsonQueryResultSchema,
	rolloverStateSchema,
	signingQueryResultSchema,
	signingStateSchema,
} from "./schemas.js";

function jsonReplacer(_key: string, value: unknown): unknown {
	if (typeof value === "bigint") {
		return value.toString();
	}
	return value;
}

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

export function writeSigningStates(db: Database, states: Record<SignatureId, SigningState>): void {
	// Clean up all prior existing signing states
	const deleteAllQuery = "DELETE FROM signing_states;";

	// Statement to insert or replace a single row
	const upsertStmt = db.prepare(`
        INSERT INTO signing_states (signatureId, stateJson)
        VALUES (?, ?)
        ON CONFLICT(signatureId) DO UPDATE SET
            stateJson = excluded.stateJson;
    `);

	// Run all operations inside a single transaction
	const signatureIds = Object.keys(states) as SignatureId[];
	db.transaction(() => {
		db.exec(deleteAllQuery);
		for (const signatureId of signatureIds) {
			const state = states[signatureId];
			const stateJson = JSON.stringify(state, jsonReplacer);
			upsertStmt.run(signatureId, stateJson);
		}
	})();
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

	constructor(path: string) {
		const db = new Sqlite3(path);
		db.exec(`
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
		const consensus = loadConsensusState(db);
		const rollover = loadRolloverState(db);
		const signing = loadSigningStates(db);
		super(consensus, { rollover, signing });

		this.#db = db;
	}

	applyDiff(diff: StateDiff): ProtocolAction[] {
		// Use the actions from the inmemory storage as a return value
		const actions = super.applyDiff(diff);
		// Sync the db with the inmemory storage
		writeConsensusState(this.#db, super.consensusState());
		const machineStates = super.machineStates();
		writeRolloverState(this.#db, machineStates.rollover);
		writeSigningStates(this.#db, machineStates.signing);
		return actions;
	}
}
