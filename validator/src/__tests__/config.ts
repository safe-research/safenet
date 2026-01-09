import { createLogger } from "../utils/logging.js";
import { createMetricsService } from "../utils/metrics.js";

const { SHIELDNET_TEST_VERBOSE } = process.env;

export const silentLogger = createLogger({ level: "silent" });
export const testLogger = createLogger({
	level: SHIELDNET_TEST_VERBOSE === "true" || SHIELDNET_TEST_VERBOSE === "1" ? "debug" : "silent",
	pretty: true,
});

export const log = testLogger.debug.bind(testLogger);

export const testMetrics = createMetricsService({ logger: silentLogger }).metrics;
