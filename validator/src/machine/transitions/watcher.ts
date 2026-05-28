import type { Database } from "better-sqlite3";
import type { PublicClient } from "viem";
import { ALL_EVENTS } from "../../types/abis.js";
import type { ProtocolConfig } from "../../types/interfaces.js";
import type { Logger } from "../../utils/logging.js";
import type { Metrics } from "../../utils/metrics.js";
import { BlockchainWatcher, type WatcherConfig } from "./blockchain_watcher.js";
import { logToTransition } from "./onchain.js";
import type { EventTransition, StateTransition } from "./types.js";

export type { WatcherConfig } from "./blockchain_watcher.js";
export type Config = Pick<ProtocolConfig, "coordinator" | "consensus" | "allowedOracles">;

export class OnchainTransitionWatcher extends BlockchainWatcher<typeof ALL_EVENTS, EventTransition> {
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
		onTransition: (transition: StateTransition) => void;
		logger: Logger;
		metrics: Metrics;
	}) {
		super({
			database,
			publicClient,
			watcherConfig,
			logger,
			metrics,
			tableName: "transition_watcher",
			address: [config.consensus, config.coordinator, ...(config.allowedOracles ?? [])],
			events: ALL_EVENTS,
			fallibleEvents: ["TransactionProposed"],
			logToTransition,
			onTransition,
		});
	}
}
