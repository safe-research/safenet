import type { Database } from "better-sqlite3";
import type { PublicClient } from "viem";
import z from "zod";
import { ALL_EVENTS } from "../../types/abis.js";
import type { ProtocolConfig } from "../../types/interfaces.js";
import type { Logger } from "../../utils/logging.js";
import { type Stop, type WatchParams, watchBlocksAndEvents } from "../../watcher/index.js";
import { logToTransition } from "./onchain.js";
import type { StateTransition } from "./types.js";

export const transitionWatcherStateSchema = z
	.object({
		chainId: z.coerce.bigint(),
		lastIndexedBlock: z.coerce.bigint(),
	})
	.optional();

export type Config = Pick<ProtocolConfig, "coordinator" | "consensus">;
export type WatcherConfig = Pick<
	WatchParams<[]>,
	| "maxReorgDepth"
	| "blockPageSize"
	| "blockPropagationDelay"
	| "blockRetryDelays"
	| "blockSingleQueryRetryCount"
	| "maxLogsPerQuery"
>;

export class OnchainTransitionWatcher {
	#logger: Logger;
	#config: Config;
	#watcher: WatcherConfig;
	#db: Database;
	#publicClient: PublicClient;
	#cleanupCallbacks: Stop[] = [];
	#onTransition: (transition: StateTransition) => void;

	constructor({
		database,
		publicClient,
		config,
		watcher,
		logger,
		onTransition,
	}: {
		database: Database;
		publicClient: PublicClient;
		config: Config;
		watcher: WatcherConfig;
		onTransition: (transition: StateTransition) => void;
		logger: Logger;
	}) {
		this.#db = database;
		this.#config = config;
		this.#watcher = watcher;
		this.#logger = logger;
		this.#publicClient = publicClient;
		this.#onTransition = onTransition;

		this.#db.exec(`
			CREATE TABLE IF NOT EXISTS transition_watcher (
				chainId INTEGER PRIMARY KEY,
				lastIndexedBlock INTEGER NOT NULL
			);
		`);
	}

	private async getLastIndexedBlock(): Promise<bigint | undefined> {
		const clientChainId = this.#publicClient.chain?.id ?? 0n;
		const stmt = this.#db.prepare("SELECT chainId, lastIndexedBlock FROM transition_watcher WHERE chainId = ?");
		const result = transitionWatcherStateSchema.parse(stmt.get(clientChainId));
		return result?.lastIndexedBlock;
	}

	updateLastIndexedBlock(block: bigint): boolean {
		const stmt = this.#db.prepare(`
			INSERT INTO transition_watcher (chainId, lastIndexedBlock)
			VALUES (@chainId, @block)
			ON CONFLICT(chainId) DO UPDATE
			SET lastIndexedBlock = excluded.lastIndexedBlock
			WHERE excluded.lastIndexedBlock >= transition_watcher.lastIndexedBlock
		`);
		const chainId = this.#publicClient.chain?.id ?? 0n;
		const info = stmt.run({ chainId, block });
		return info.changes > 0;
	}

	handleTransition(transition: StateTransition) {
		try {
			if (!this.updateLastIndexedBlock(transition.block)) {
				this.#logger.warn("Received an out-of-order transition", { transition });
				return;
			}
			this.#onTransition(transition);
		} catch (e: unknown) {
			const err = e instanceof Error ? e : new Error(`unknown error: ${e}`);
			this.#logger.error("An error occurred handling a state transition:", err);
		}
	}

	async start() {
		const blockTime = this.#publicClient.chain?.blockTime;
		if (blockTime === undefined) {
			throw new Error("chain missing block time configuration");
		}

		const lastIndexedBlock = (await this.getLastIndexedBlock()) ?? null;
		const stop = await watchBlocksAndEvents({
			logger: this.#logger,
			client: this.#publicClient,
			...this.#watcher,
			lastIndexedBlock,
			blockTime,
			address: [this.#config.consensus, this.#config.coordinator],
			events: ALL_EVENTS,
			fallibleEvents: ["TransactionProposed"],
			handler: (update) => {
				switch (update.type) {
					case "watcher_update_warp_to_block": {
						// Note that we don't explicitely handle warping in our state machine,
						// instead if any events are found in the log range, the state machine is
						// updated to the correct block accordingly.
						this.#logger.debug(`warping to block ${update.toBlock}`);
						break;
					}
					case "watcher_update_uncle_block": {
						this.#logger.warn("reorg detected, but currently not supported", { update });
						break;
					}
					case "watcher_update_new_block": {
						this.handleTransition({ id: "block_new", block: update.blockNumber });
						break;
					}
					case "watcher_update_new_logs": {
						for (const log of update.logs) {
							this.handleTransition(logToTransition(log));
						}
						break;
					}
				}
			},
		});
		this.#cleanupCallbacks.push(stop);
	}

	async stop() {
		const cleanupCallbacks = this.#cleanupCallbacks;
		this.#cleanupCallbacks = [];
		await Promise.all(cleanupCallbacks.map(callback => callback()));
	}
}
