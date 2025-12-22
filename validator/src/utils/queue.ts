import type { Database } from "better-sqlite3";
import z, { type ZodType } from "zod";
import { jsonReplacer } from "./json.js";

type QueueEntry<T> = {
	prev?: QueueEntry<T>;
	next?: QueueEntry<T>;
	element: T;
};

const queueSQLiteSchema = z.object({
	id: z.number().nonnegative(),
	payloadJson: z.string(),
});

// FIFO tyle queue
export type Queue<T> = {
	// Insert at the beginning
	push(element: T): void;
	// Peek at the next element
	peek(): T | undefined;
	// Pop from the end
	pop(): T | undefined;
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

	push(element: T): void {
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
	pop(): T | undefined {
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

export class InMemoryQueue<T> implements Queue<T> {
	#head?: QueueEntry<T>;
	#tail?: QueueEntry<T>;

	push(element: T) {
		const entry = {
			next: this.#head,
			element,
		};
		if (this.#head !== undefined) {
			this.#head.prev = entry;
		}

		this.#head = entry;
		if (this.#tail === undefined) {
			this.#tail = entry;
		}
	}

	peek(): T | undefined {
		return this.#tail?.element;
	}

	pop(): T | undefined {
		const entry = this.#tail;
		this.#tail = entry?.prev;
		if (entry?.prev === undefined) {
			this.#head = undefined;
		} else {
			entry.prev.next = undefined;
		}
		return entry?.element;
	}
}
