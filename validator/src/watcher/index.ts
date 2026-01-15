/**
 * Watching for blocks and on-chain events.
 */

import type { Prettify } from "viem";
import type { Logger } from "../utils/logging.js";
import {
	type Client as BlockClient,
	type BlockUpdate,
	BlockWatcher,
	type CreateParams as BlockWatcherCreateParams,
} from "./blocks.js";
import {
	type Client as EventClient,
	type Events,
	EventWatcher,
	type ConstructorParams as EventWatcherConstructorParams,
	type Log,
} from "./events.js";

/**
 * A watcher update.
 */
export type Update<E extends Events> = BlockUpdate | { type: "watcher_update_new_logs"; logs: Log<E>[] };

export type Handler<E extends Events> = (update: Update<E>) => void;
export type Unsubscribe = () => void;
export type Stop = () => Promise<void>;
export type CreateParams<E extends Events> = Prettify<
	{ client: BlockClient & EventClient } & BlockWatcherCreateParams & EventWatcherConstructorParams<E>
>;
export type WatchParams<E extends Events> = Prettify<{ handler: Handler<E> } & CreateParams<E>>;

/**
 * Watcher for blocks and events used to drive the validator.
 */
export class Watcher<E extends Events> {
	#logger: Logger;
	#blocks: BlockWatcher;
	#events: EventWatcher<E>;
	#handlers: Map<symbol, Handler<E>>;
	#worker: Promise<void>;
	#running: boolean;

	private constructor(logger: Logger, blocks: BlockWatcher, events: EventWatcher<E>) {
		this.#logger = logger;
		this.#blocks = blocks;
		this.#events = events;
		this.#handlers = new Map();
		this.#worker = Promise.resolve();
		this.#running = false;
	}

	async #run() {
		while (this.#running) {
			try {
				while (true /* logs !== null */) {
					const logs = await this.#events.next();
					if (logs === null) {
						break;
					}

					// Trigger an update if we have some logs.
					if (logs.length > 0) {
						this.#onUpdate({ type: "watcher_update_new_logs", logs });
					}

					// Check in between updates to make sure that we don't wait too long to stop
					// if we are in the middle of warping over a large block range.
					if (!this.#running) {
						return;
					}
				}

				const update = await this.#blocks.next();
				this.#events.onBlockUpdate(update);
				this.#onUpdate(update);
			} catch (error) {
				this.#logger.warn("internal watcher error", { error });
			}
		}
	}

	#onUpdate(update: Update<E>) {
		for (const handler of this.#handlers.values()) {
			try {
				handler(update);
			} catch (error) {
				this.#logger.warn("watcher handler failed", { error, update });
			}
		}
	}

	/**
	 * Start the watcher.
	 */
	start(): void {
		if (this.#running) {
			throw new Error("already started");
		}
		this.#running = true;
		this.#worker = this.#run();
	}

	/**
	 * Stop the watcher.
	 */
	stop(): Promise<void> {
		if (!this.#running) {
			throw new Error("already stopped");
		}
		this.#running = false;
		return this.#worker;
	}

	/**
	 * Subscribe to watcher updates.
	 *
	 * This should be called before `start`, otherwise some updates may be missed. Returns a closure
	 * that can be called in order to unsubscribe.
	 */
	subscribe(handler: Handler<E>): Unsubscribe {
		const id = Symbol();
		this.#handlers.set(id, handler);
		return () => {
			this.#handlers.delete(id);
		};
	}

	/**
	 * Create and initialize a new watcher.
	 */
	static async create<E extends Events>(params: CreateParams<E>): Promise<Watcher<E>> {
		const logger = params.logger;
		const blocks = await BlockWatcher.create(params);
		const events = new EventWatcher(params);
		return new Watcher(logger, blocks, events);
	}
}

export const watchBlocksAndEvents = async <E extends Events>(params: WatchParams<E>): Promise<Stop> => {
	const watcher = await Watcher.create(params);
	const unsubscribe = watcher.subscribe(params.handler);
	watcher.start();
	return () => {
		unsubscribe();
		return watcher.stop();
	};
};
