import type { Database } from "better-sqlite3";
import type { Chain, PublicClient, Transport } from "viem";
import { SqliteTxStorage } from "../consensus/protocol/sqlite.js";
import { GasFeeEstimator, TransactionManager } from "../consensus/protocol/transaction.js";
import type { WatcherConfig } from "../shared/watcher.js";
import type { ValidatorAccount } from "../types/account.js";
import type { Logger } from "../utils/logging.js";
import type { Metrics } from "../utils/metrics.js";
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
import type { SentinelOracleTransition } from "./transitions.js";
import type { SentinelConfig, SentinelStateDiff } from "./types.js";
import { SentinelTransitionWatcher } from "./watcher.js";

export class SentinelService {
	#logger: Logger;
	#config: SentinelConfig;
	#detector: Detector;
	#storage: SentinelStateStorage;
	#protocol: SentinelProtocol;
	#watcher: SentinelTransitionWatcher;

	constructor({
		account,
		publicClient,
		config,
		detector,
		logger,
		metrics,
		watcherConfig,
		database,
	}: {
		account: ValidatorAccount;
		publicClient: PublicClient<Transport, Chain>;
		config: SentinelConfig;
		detector: Detector;
		logger: Logger;
		metrics: Metrics;
		watcherConfig: WatcherConfig;
		database: Database;
	}) {
		this.#logger = logger;
		this.#config = config;
		this.#detector = detector;
		this.#storage = new SentinelStateStorage(database);

		const actionQueue = new SentinelActionQueue(database);
		const txStorage = new SqliteTxStorage(database);
		const gasFeeEstimator = new GasFeeEstimator(publicClient);
		const txManager = new TransactionManager({
			publicClient,
			account,
			gasFeeEstimator,
			txStorage,
			logger,
		});
		this.#protocol = new SentinelProtocol(config.oracle, config.feeToken, actionQueue, txManager, logger);
		this.#watcher = new SentinelTransitionWatcher({
			database,
			publicClient,
			config,
			watcherConfig,
			logger,
			metrics,
			onTransition: (transition) => {
				if (transition.id === "block_new") {
					gasFeeEstimator.invalidate();
					txManager.triggerPendingCheck(transition.block);
					this.#processBlock(transition.block);
				} else {
					this.#processLog(transition);
				}
			},
		});
	}

	async start(): Promise<void> {
		this.#protocol.drain();
		await this.#watcher.start();
		this.#logger.notice("SentinelService started", { sentinelOracle: this.#config.oracle });
	}

	async stop(): Promise<void> {
		await this.#watcher.stop();
		this.#logger.notice("SentinelService stopped");
	}

	#processLog(transition: SentinelOracleTransition): void {
		this.#applyDiffs(this.#handleLog(transition));
	}

	async #processBlock(blockNumber: bigint): Promise<void> {
		const diffs = handleBlockAdvance(this.#storage.requests(), blockNumber, this.#config);
		this.#applyDiffs(diffs);
	}

	#applyDiffs(diffs: SentinelStateDiff[]): void {
		for (const diff of diffs) {
			const actions = this.#storage.applyDiff(diff);
			for (const action of actions) this.#protocol.process(action);
		}
	}

	#handleLog(transition: SentinelOracleTransition): SentinelStateDiff[] {
		this.#logger.debug(`Handle event ${transition.id}`, { transition });
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
