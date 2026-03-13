import type { Database } from "better-sqlite3";
import z, { type ZodType } from "zod";
import { jsonReplacer } from "./json.js";

const queueSQLiteSchema = z.object({
	id: z.number().nonnegative(),
	payloadJson: z.string(),
});

// FIFO style queue
export type Queue<T> = {
	// Add an item to the queue
	enqueue(element: T): void;
	// Peek at the next element
	peek(): T | undefined;
	// Remove the next item from the queue
	dequeue(): T | undefined;
};

export class SqliteQueue<T> implements Queue<T> {
	#schema: ZodType<T>;
	#db: Database;
	#name: string;

	constructor(schema: ZodType<T>, database: Database, name: string) {
		this.#schema = schema;
		this.#db = database;
		this.#name = name;

		this.#db.exec(`
			CREATE TABLE IF NOT EXISTS queue_${name} (
				id INTEGER PRIMARY KEY,
				payloadJson TEXT NOT NULL
			);
		`);
	}

	enqueue(element: T): void {
		const payloadJson = JSON.stringify(element, jsonReplacer);
		this.#db
			.prepare(`
			INSERT INTO queue_${this.#name} (payloadJson)
			VALUES (?)
		`)
			.run(payloadJson);
	}

	peek(): T | undefined {
		const messageRow = this.#db
			.prepare(`
			SELECT id, payloadJson
			FROM queue_${this.#name}
			ORDER BY id ASC
			LIMIT 1;
		`)
			.get();

		if (!messageRow) {
			return undefined; // Queue is empty
		}
		const message = queueSQLiteSchema.parse(messageRow);
		const payloadJson = JSON.parse(message.payloadJson);
		return this.#schema.parse(payloadJson);
	}

	dequeue(): T | undefined {
		return this.#db.transaction(() => {
			// Step 1: Select the oldest message
			const messageRow = this.#db
				.prepare(`
				SELECT id, payloadJson
				FROM queue_${this.#name}
				ORDER BY id ASC
				LIMIT 1;
			`)
				.get();

			if (!messageRow) {
				return undefined; // Queue is empty
			}

			const message = queueSQLiteSchema.parse(messageRow);

			// Step 2: Delete the message using its ID
			this.#db
				.prepare(`
				DELETE FROM queue_${this.#name}
				WHERE id = ?;
			`)
				.run(message.id);

			// Step 3: Parse the payload json
			const payloadJson = JSON.parse(message.payloadJson);

			// Step 4: Validate the payload json
			return this.#schema.parse(payloadJson);
		})();
	}
}
