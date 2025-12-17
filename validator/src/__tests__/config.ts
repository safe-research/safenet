import type { Address } from "viem";
import { InMemoryClientStorage } from "../consensus/storage/inmemory.js";
import { SqliteClientStorage } from "../consensus/storage/sqlite.js";
import { InMemoryStateStorage } from "../machine/storage/inmemory.js";
import { SqliteStateStorage } from "../machine/storage/sqlite.js";
import { createLogger } from "../utils/logging.js";
import { createMetricsService } from "../utils/metrics.js";

const { SHIELDNET_TEST_VERBOSE, SHIELDNET_TEST_STORAGE } = process.env;

export const silentLogger = createLogger({ level: "silent" });
export const testLogger = createLogger({
	level: SHIELDNET_TEST_VERBOSE === "true" || SHIELDNET_TEST_VERBOSE === "1" ? "debug" : "silent",
	pretty: true,
});

export const log = testLogger.debug.bind(testLogger);

export const testMetrics = createMetricsService({ logger: silentLogger }).metrics;

export const createClientStorage =
	SHIELDNET_TEST_STORAGE === "sqlite"
		? (account: Address) => new SqliteClientStorage(account, ":memory:")
		: (account: Address) => new InMemoryClientStorage(account);

export const createStateStorage =
	SHIELDNET_TEST_STORAGE === "sqlite" ? () => new SqliteStateStorage(":memory:") : () => new InMemoryStateStorage();
