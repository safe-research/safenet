import type { Database } from "better-sqlite3";
import type { Chain, PublicClient, Transport } from "viem";
import { SqliteTxStorage } from "../consensus/protocol/sqlite.js";
import { GasFeeEstimator, TransactionManager } from "../consensus/protocol/transaction.js";
import type { WatcherConfig } from "../machine/transitions/watcher.js";
import type { ValidatorAccount } from "../types/account.js";
import { formatError } from "../utils/errors.js";
import type { Logger } from "../utils/logging.js";
import { type Stop, watchBlocksAndEvents } from "../watcher/index.js";
import type { Detector } from "./detector.js";
import {
	handleBlockAdvance,
	handleCommitted,
	handleNewRequest,
	handleOracleTransactionProposed,
	handleResolved,
} from "./handlers.js";
import { SentinelActionQueue, SentinelProtocol } from "./protocol.js";
import { SentinelStateStorage } from "./storage.js";
import { logToTransition, SENTINEL_ALL_EVENTS, type SentinelOracleTransition } from "./transitions.js";
import type { SentinelConfig, SentinelStateDiff } from "./types.js";

export class SentinelService {
	#logger: Logger;
	#publicClient: PublicClient<Transport, Chain>;
	#account: ValidatorAccount;
	#config: SentinelConfig;
	#detector: Detector;
	#watcherConfig: WatcherConfig;
	#db: Database;
	#storage: SentinelStateStorage;
	#actionQueue: SentinelActionQueue;
	#stop: Stop | null = null;

	constructor({
		account,
		publicClient,
		config,
		detector,
		logger,
		watcherConfig,
		database,
	}: {
		account: ValidatorAccount;
		publicClient: PublicClient<Transport, Chain>;
		config: SentinelConfig;
		detector: Detector;
		logger: Logger;
		watcherConfig: WatcherConfig;
		database: Database;
	}) {
		this.#account = account;
		this.#publicClient = publicClient;
		this.#config = config;
		this.#detector = detector;
		this.#logger = logger;
		this.#watcherConfig = watcherConfig;
		this.#db = database;
		this.#storage = new SentinelStateStorage(this.#db);
		this.#actionQueue = new SentinelActionQueue(this.#db);
	}

	async start(): Promise<void> {
		if (this.#stop !== null) {
			throw new Error("SentinelService already started");
		}
		const txStorage = new SqliteTxStorage(this.#db);
		const gasFeeEstimator = new GasFeeEstimator(this.#publicClient);
		const txManager = new TransactionManager({
			publicClient: this.#publicClient,
			account: this.#account,
			gasFeeEstimator,
			txStorage,
			logger: this.#logger,
		});
		const protocol = new SentinelProtocol(
			this.#config.oracle,
			this.#config.feeToken,
			this.#actionQueue,
			txManager,
			this.#logger,
		);
		protocol.drain();

		const blockTime = this.#watcherConfig.blockTimeOverride ?? this.#publicClient.chain?.blockTime;
		if (blockTime === undefined) {
			throw new Error("SentinelService: chain missing block time configuration");
		}

		let handlerChain: Promise<unknown> = Promise.resolve();
		this.#stop = await watchBlocksAndEvents({
			logger: this.#logger,
			client: this.#publicClient,
			...this.#watcherConfig,
			lastIndexedBlock: null,
			blockTime,
			address: [this.#config.oracle, this.#config.consensus],
			events: SENTINEL_ALL_EVENTS,
			handler: (update) => {
				if (update.type === "watcher_update_new_logs") {
					for (const log of update.logs) {
						handlerChain = handlerChain
							.then(() => this.#processLog(protocol, logToTransition(log)))
							.catch((err) => this.#logger.error("SentinelService: error processing log", { error: formatError(err) }));
					}
				} else if (update.type === "watcher_update_new_block") {
					txManager.triggerPendingCheck(update.blockNumber);
					handlerChain = handlerChain
						.then(() => this.#processBlock(protocol, update.blockNumber))
						.catch((err) => this.#logger.error("SentinelService: error processing block", { error: formatError(err) }));
				}
			},
		});
		this.#logger.notice("SentinelService started", { sentinelOracle: this.#config.oracle });
	}

	async stop(): Promise<void> {
		if (this.#stop === null) {
			throw new Error("SentinelService not started");
		}
		await this.#stop();
		this.#stop = null;
		this.#logger.notice("SentinelService stopped");
	}

	#processLog(protocol: SentinelProtocol, transition: SentinelOracleTransition): void {
		this.#applyDiffs(protocol, this.#handleLog(transition));
	}

	async #processBlock(protocol: SentinelProtocol, blockNumber: bigint): Promise<void> {
		const diffs = handleBlockAdvance(this.#storage.requests(), blockNumber, this.#config);
		this.#applyDiffs(protocol, diffs);
	}

	#applyDiffs(protocol: SentinelProtocol, diffs: SentinelStateDiff[]): void {
		for (const diff of diffs) {
			const actions = this.#storage.applyDiff(diff);
			for (const action of actions) protocol.process(action);
		}
	}

	#handleLog(transition: SentinelOracleTransition): SentinelStateDiff[] {
		switch (transition.id) {
			case "event_oracle_transaction_proposed": {
				return [handleOracleTransactionProposed(transition, this.#config, this.#detector)];
			}
			case "sentinel_event_new_request": {
				return [handleNewRequest(this.#storage.requests(), transition, this.#logger)];
			}
			case "sentinel_event_committed": {
				return [handleCommitted(this.#storage.requests(), transition, this.#config)];
			}
			case "sentinel_event_oracle_result": {
				return [handleResolved(this.#storage.requests(), transition)];
			}
		}
	}
}
