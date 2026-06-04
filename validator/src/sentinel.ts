import Sqlite3 from "better-sqlite3";
import dotenv from "dotenv";
import { type ChainFees, createPublicClient, extractChain, http, webSocket } from "viem";
import { createNonceManager, privateKeyToAccount } from "viem/accounts";
import { jsonRpc } from "viem/nonce";
import { z } from "zod";
import { createDetector } from "./sentinel/detector.js";
import { SentinelService } from "./sentinel/service.js";
import type { SentinelConfig } from "./sentinel/types.js";
import type { WatcherConfig } from "./shared/watcher.js";
import { supportedChains } from "./types/chains.js";
import { sentinelConfigSchema } from "./types/schemas.js";
import { formatError } from "./utils/errors.js";
import { createLogger } from "./utils/logging.js";
import { createMetricsService } from "./utils/metrics.js";
import { withTracing } from "./utils/transport.js";
import { COMMIT_SHA } from "./version.js";

dotenv.config({ quiet: true });

const result = sentinelConfigSchema.safeParse(process.env);
if (!result.success) {
	console.error("Invalid environment variable configuration:", z.treeifyError(result.error));
	process.exit(1);
}

const cfg = result.data;

const logger = createLogger({
	level: cfg.LOG_LEVEL,
	pretty: process.stdout.isTTY,
});

const account = privateKeyToAccount(cfg.PRIVATE_KEY, {
	nonceManager: createNonceManager({ source: jsonRpc() }),
});
logger.notice(`Using sentinel account ${account.address}`);

const metrics = createMetricsService({
	logger,
	host: cfg.METRICS_HOST,
	port: cfg.METRICS_PORT,
	commit: COMMIT_SHA,
});

const transport = withTracing(cfg.RPC_URL.startsWith("wss") ? webSocket(cfg.RPC_URL) : http(cfg.RPC_URL), {
	logger,
	metrics: metrics.metrics,
});

const fees: ChainFees = {
	baseFeeMultiplier: cfg.BASE_FEE_MULTIPLIER ?? 2,
	maxPriorityFeePerGas: cfg.PRIORITY_FEE_PER_GAS,
};

const chain = {
	...extractChain({ chains: supportedChains, id: cfg.CHAIN_ID }),
	fees,
};

const publicClient = createPublicClient({ chain, transport });

const watcherConfig: WatcherConfig = {
	blockTimeOverride: cfg.BLOCK_TIME_OVERRIDE,
	maxReorgDepth: cfg.MAX_REORG_DEPTH ?? 5,
	blockPageSize: cfg.BLOCK_PAGE_SIZE,
	blockAllLogsQueryRetryCount: cfg.BLOCK_ALL_LOGS_QUERY_RETRY_COUNT,
	blockSingleQueryRetryCount: cfg.BLOCK_SINGLE_QUERY_RETRY_COUNT,
	maxLogsPerQuery: cfg.MAX_LOGS_PER_QUERY,
};

const db = new Sqlite3(cfg.STORAGE_FILE ?? ":memory:");

const sentinelConfig: SentinelConfig = {
	account: account.address,
	oracle: cfg.SENTINEL_ORACLE_ADDRESS,
	feeToken: cfg.SENTINEL_ORACLE_FEE_TOKEN,
	consensus: cfg.CONSENSUS_ADDRESS,
	chainId: BigInt(cfg.CHAIN_ID),
	votingWindow: cfg.SENTINEL_VOTING_WINDOW,
};

const service = new SentinelService({
	account,
	publicClient,
	config: sentinelConfig,
	detector: createDetector(cfg.SENTINEL_BLOCKLIST),
	logger,
	watcherConfig,
	database: db,
	metrics: metrics.metrics,
});

let shuttingDown = false;
const shutdown = async () => {
	if (shuttingDown) return;
	shuttingDown = true;
	logger.notice("Shutting down sentinel service...");
	try {
		await Promise.all([service.stop(), metrics.stop()]);
		db.close();
	} catch (error: unknown) {
		logger.error("Error during shutdown.", { error: formatError(error) });
	}
	process.exit(0);
};
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

Promise.all([service.start(), metrics.start()]).catch((error: unknown) => {
	logger.error("Sentinel service failed to start.", { error: formatError(error) });
	process.exit(1);
});

export default {};
