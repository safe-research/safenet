import Sqlite3, { type Database } from "better-sqlite3";
import type { Address } from "viem";
import { InMemoryClientStorage } from "../consensus/storage/inmemory.js";
import { SqliteClientStorage } from "../consensus/storage/sqlite.js";
import { InMemoryStateStorage } from "../machine/storage/inmemory.js";
import { SqliteStateStorage } from "../machine/storage/sqlite.js";
import { createLogger } from "../utils/logging.js";
import { createMetricsService } from "../utils/metrics.js";

const { SAFENET_TEST_VERBOSE, SAFENET_TEST_STORAGE } = process.env;

export const silentLogger = createLogger({ level: "silent" });
export const testLogger = createLogger({
	level: SAFENET_TEST_VERBOSE === "true" || SAFENET_TEST_VERBOSE === "1" ? "debug" : "silent",
	pretty: true,
});

export const log = testLogger.debug.bind(testLogger);

export const testMetrics = createMetricsService({ logger: silentLogger }).metrics;

export const createClientStorage =
	SAFENET_TEST_STORAGE === "sqlite"
		? (account: Address, database?: Database) => new SqliteClientStorage(account, database ?? new Sqlite3(":memory:"))
		: (account: Address) => new InMemoryClientStorage(account);

export const createStateStorage =
	SAFENET_TEST_STORAGE === "sqlite"
		? (database?: Database) => new SqliteStateStorage(database ?? new Sqlite3(":memory:"))
		: () => new InMemoryStateStorage();
