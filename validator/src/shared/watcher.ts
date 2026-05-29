import type { Database } from "better-sqlite3";
import type { Address, Prettify, PublicClient } from "viem";
import z from "zod";
import type { NewBlock } from "../machine/transitions/types.js";
import { formatError } from "../utils/errors.js";
import type { Logger } from "../utils/logging.js";
import type { Metrics } from "../utils/metrics.js";
import type { Events, Log } from "../watcher/events.js";
import { type Stop, type Update, type WatchParams, watchBlocksAndEvents } from "../watcher/index.js";

const watcherStateSchema = z
	.object({
		chainId: z.coerce.bigint(),
		lastIndexedBlock: z.coerce.bigint(),
	})
	.optional();

export type WatcherConfig = Prettify<
	{ blockTimeOverride?: number } & Pick<
		WatchParams<[]>,
		| "maxReorgDepth"
		| "blockPageSize"
		| "blockPropagationDelay"
		| "blockRetryDelays"
		| "blockAllLogsQueryRetryCount"
		| "blockSingleQueryRetryCount"
		| "maxLogsPerQuery"
		| "backoffDelays"
	>
>;

export abstract class BlockchainWatcher<E extends Events, L> {
	#logger: Logger;
	#metrics: Metrics;
	#watcherConfig: WatcherConfig;
	#db: Database;
	#publicClient: PublicClient;
	#tableName: string;
	#filter: { address: Address[]; events: E; fallibleEvents: string[] };
	#logToTransition: (log: Log<E>) => L;
	#onTransition: (transition: NewBlock | L) => void;
	#stop: Stop | null = null;

	constructor({
		database,
		publicClient,
		watcherConfig,
		logger,
		metrics,
		tableName,
		address,
		events,
		fallibleEvents,
		logToTransition,
		onTransition,
	}: {
		database: Database;
		publicClient: PublicClient;
		watcherConfig: WatcherConfig;
		logger: Logger;
		metrics: Metrics;
		tableName: string;
		address: Address[];
		events: E;
		fallibleEvents: string[];
		logToTransition: (log: Log<E>) => L;
		onTransition: (transition: NewBlock | L) => void;
	}) {
		this.#db = database;
		this.#watcherConfig = watcherConfig;
		this.#logger = logger;
		this.#metrics = metrics;
		this.#publicClient = publicClient;
		this.#tableName = tableName;
		this.#filter = { address, events, fallibleEvents };
		this.#logToTransition = logToTransition;
		this.#onTransition = onTransition;

		this.#db.exec(`
			CREATE TABLE IF NOT EXISTS ${tableName} (
				chainId INTEGER PRIMARY KEY,
				lastIndexedBlock INTEGER NOT NULL
			);
		`);
	}

	async #getLastIndexedBlock(): Promise<bigint | undefined> {
		const clientChainId = this.#publicClient.chain?.id ?? 0n;
		const stmt = this.#db.prepare(`SELECT chainId, lastIndexedBlock FROM ${this.#tableName} WHERE chainId = ?`);
		const result = watcherStateSchema.parse(stmt.get(clientChainId));
		return result?.lastIndexedBlock;
	}

	#updateLastIndexedBlock(block: bigint): boolean {
		const stmt = this.#db.prepare(`
			INSERT INTO ${this.#tableName} (chainId, lastIndexedBlock)
			VALUES (@chainId, @block)
			ON CONFLICT(chainId) DO UPDATE
			SET lastIndexedBlock = excluded.lastIndexedBlock
			WHERE excluded.lastIndexedBlock >= ${this.#tableName}.lastIndexedBlock
		`);
		const chainId = this.#publicClient.chain?.id ?? 0n;
		const info = stmt.run({ chainId, block });
		return info.changes > 0;
	}

	#handleTransition(t: NewBlock | L, block: bigint): void {
		try {
			if (!this.#updateLastIndexedBlock(block)) {
				this.#logger.warn("Received an out-of-order transition.", { transition: t });
				return;
			}
			this.#onTransition(t);
		} catch (error) {
			this.#logger.error("An error occurred handling a state transition.", { error: formatError(error) });
		}
	}

	#handleUpdate(update: Update<E>): void {
		switch (update.type) {
			case "watcher_update_warp_to_block": {
				this.#metrics.blockNumber.labels({ status: "seen" }).set(Number(update.toBlock));
				this.#metrics.eventIndex.labels({ status: "seen" }).set(-1);
				// Note that we don't explicitly handle warping in our state machine,
				// instead if any events are found in the log range, the state machine is
				// updated to the correct block accordingly.
				this.#logger.debug(`warping to block ${update.toBlock}`);
				break;
			}
			case "watcher_update_uncle_block": {
				this.#metrics.reorgs.inc();
				this.#logger.warn("Reorg detected, but currently not supported.", { update });
				break;
			}
			case "watcher_update_new_block": {
				this.#metrics.blockNumber.labels({ status: "seen" }).set(Number(update.blockNumber));
				this.#metrics.eventIndex.labels({ status: "seen" }).set(-1);
				this.#handleTransition({ id: "block_new", block: update.blockNumber }, update.blockNumber);
				break;
			}
			case "watcher_update_new_logs": {
				this.#metrics.eventIndex.labels({ status: "seen" }).set(update.logs.at(-1)?.logIndex ?? -1);
				for (const log of update.logs) {
					this.#handleTransition(this.#logToTransition(log), log.blockNumber);
				}
				break;
			}
		}
	}

	async start() {
		if (this.#stop !== null) {
			throw new Error("already started");
		}

		const blockTime = this.#watcherConfig.blockTimeOverride ?? this.#publicClient.chain?.blockTime;
		if (blockTime === undefined) {
			throw new Error("chain missing block time configuration");
		}

		const lastIndexedBlock = (await this.#getLastIndexedBlock()) ?? null;
		this.#stop = await watchBlocksAndEvents({
			logger: this.#logger,
			client: this.#publicClient,
			...this.#watcherConfig,
			lastIndexedBlock,
			blockTime,
			...this.#filter,
			handler: (update) => this.#handleUpdate(update),
		});
	}

	async stop() {
		if (this.#stop === null) {
			throw new Error("already stopped");
		}

		const stop = this.#stop;
		this.#stop = null;
		await stop();
	}
}
