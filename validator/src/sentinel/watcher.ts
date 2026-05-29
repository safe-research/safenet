import type { Database } from "better-sqlite3";
import type { PublicClient } from "viem";
import type { NewBlock } from "../machine/transitions/types.js";
import { BlockchainWatcher, type WatcherConfig } from "../shared/watcher.js";
import type { Logger } from "../utils/logging.js";
import type { Metrics } from "../utils/metrics.js";
import { logToTransition, SENTINEL_ALL_EVENTS, type SentinelOracleTransition } from "./transitions.js";
import type { SentinelConfig } from "./types.js";

export type Config = Pick<SentinelConfig, "oracle" | "consensus">;

export class SentinelTransitionWatcher extends BlockchainWatcher<typeof SENTINEL_ALL_EVENTS, SentinelOracleTransition> {
	constructor({
		database,
		publicClient,
		config,
		watcherConfig,
		logger,
		metrics,
		onTransition,
	}: {
		database: Database;
		publicClient: PublicClient;
		config: Config;
		watcherConfig: WatcherConfig;
		onTransition: (transition: NewBlock | SentinelOracleTransition) => void;
		logger: Logger;
		metrics: Metrics;
	}) {
		super({
			database,
			publicClient,
			watcherConfig,
			logger,
			metrics,
			tableName: "sentinel_transition_watcher",
			address: [config.oracle, config.consensus],
			events: SENTINEL_ALL_EVENTS,
			fallibleEvents: [],
			logToTransition,
			onTransition,
		});
	}
}
