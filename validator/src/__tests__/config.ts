import { createLogger } from "../utils/logging.js";
import { createMetricsService } from "../utils/metrics.js";

const { SAFENET_TEST_VERBOSE } = process.env;

export const envFlag = (value: string | undefined) => value === "true" || value === "1";

export const isVerbose = () => envFlag(SAFENET_TEST_VERBOSE);
export const silentLogger = createLogger({ level: "silent" });
export const testLogger = createLogger({
	level: isVerbose() ? "debug" : "silent",
	pretty: true,
});

export const log = testLogger.debug.bind(testLogger);

export const testMetrics = createMetricsService({ logger: silentLogger }).metrics;
