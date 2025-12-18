import Sqlite3, { type Database } from "better-sqlite3";
import type { PublicClient } from "viem";
import z from "zod";
import { CONSENSUS_EVENTS, COORDINATOR_EVENTS } from "../../types/abis.js";
import type { ProtocolConfig } from "../../types/interfaces.js";
import type { Logger } from "../../utils/logging.js";
import { logToTransition } from "./onchain.js";
import type { StateTransition } from "./types.js";

export const transitionWatcherStateSchema = z
	.object({
		chainId: z.coerce.bigint(),
		lastIndexedBlock: z.coerce.bigint(),
	})
	.optional();

export class OnchainTransitionWatcher {
	#logger: Logger;
	#config: Pick<ProtocolConfig, "consensus" | "coordinator">;
	#db: Database;
	#publicClient: PublicClient;
	#cleanupCallbacks: (() => void)[] = [];
	#onTransition: (transition: StateTransition) => void;

	constructor({
		dbPath,
		publicClient,
		config,
		logger,
		onTransition,
	}: {
		dbPath: string;
		publicClient: PublicClient;
		config: Pick<ProtocolConfig, "consensus" | "coordinator">;
		onTransition: (transition: StateTransition) => void;
		logger: Logger;
	}) {
		const db = new Sqlite3(dbPath);
		db.exec(`
            CREATE TABLE IF NOT EXISTS transition_watcher (
                chainId INTEGER PRIMARY KEY,
                lastIndexedBlock INTEGER NOT NULL
            );
        `);
		this.#db = db;
		this.#config = config;
		this.#logger = logger;
		this.#publicClient = publicClient;
		this.#onTransition = onTransition;
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
			if (this.updateLastIndexedBlock(transition.block)) {
				// Only trigger callback for valid transitions
				this.#onTransition(transition);
			}
		} catch (e: unknown) {
			const err = e instanceof Error ? e : new Error(`unknown error: ${e}`);
			this.#logger.error("an error occurred handling a state transition:", err);
		}
	}

	async start() {
		const lastIndexedBlock = await this.getLastIndexedBlock();
		this.#cleanupCallbacks.push(
			this.#publicClient.watchContractEvent({
				address: [this.#config.consensus, this.#config.coordinator],
				abi: [...CONSENSUS_EVENTS, ...COORDINATOR_EVENTS],
				fromBlock: lastIndexedBlock ? lastIndexedBlock + 1n : undefined,
				onLogs: async (logs) => {
					logs.sort((left, right) => {
						if (left.blockNumber !== right.blockNumber) {
							return left.blockNumber < right.blockNumber ? -1 : 1;
						}
						return left.logIndex - right.logIndex;
					});
					for (const log of logs) {
						const transition = logToTransition(log.blockNumber, log.logIndex, log.eventName, log.args);
						if (transition === undefined) {
							this.#logger.info(`Unknown log: ${log.eventName}`);
							continue;
						}
						this.handleTransition(transition);
					}
				},
				onError: (err) => this.#logger.error("contract event watcher error:", err),
			}),
		);
		this.#cleanupCallbacks.push(
			this.#publicClient.watchBlockNumber({
				onBlockNumber: (block) => {
					// we delay the processing to avoid potential race conditions for now
					setTimeout(() => {
						this.handleTransition({
							id: "block_new",
							block,
						});
					}, 2000);
				},
				onError: (err) => this.#logger.error("block number watcher error:", err),
			}),
		);
	}

	stop() {
		const cleanupCallbacks = this.#cleanupCallbacks;
		this.#cleanupCallbacks = [];
		for (const callback of cleanupCallbacks) {
			callback();
		}
	}
}
