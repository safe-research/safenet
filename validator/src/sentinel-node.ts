import Sqlite3 from "better-sqlite3";
import dotenv from "dotenv";
import { type ChainFees, createPublicClient, extractChain, http, webSocket } from "viem";
import { createNonceManager, privateKeyToAccount } from "viem/accounts";
import { jsonRpc } from "viem/nonce";
import { z } from "zod";
import type { WatcherConfig } from "./machine/transitions/watcher.js";
import { createDetector } from "./sentinel/detector.js";
import { SentinelService } from "./sentinel/service.js";
import { supportedChains } from "./types/chains.js";
import {
	checkedAddressSchema,
	hexBytes32Schema,
	metricsConfigSchema,
	sentinelConfigSchema,
	submissionConfigSchema,
	supportedChainsSchema,
	watcherConfigSchema,
} from "./types/schemas.js";
import { formatError } from "./utils/errors.js";
import { createLogger } from "./utils/logging.js";
import { createMetricsService } from "./utils/metrics.js";
import { withTracing } from "./utils/transport.js";
import { COMMIT_SHA } from "./version.js";

dotenv.config({ quiet: true });

const sentinelNodeConfigSchema = z.object({
	...metricsConfigSchema.shape,
	...watcherConfigSchema.shape,
	...submissionConfigSchema.shape,
	...sentinelConfigSchema.shape,
	RPC_URL: z.url(),
	CHAIN_ID: supportedChainsSchema,
	PRIVATE_KEY: hexBytes32Schema,
	CONSENSUS_ADDRESS: checkedAddressSchema,
});

const result = sentinelNodeConfigSchema.safeParse(process.env);
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

const sentinelConfig = {
	account: account.address,
	oracle: cfg.SENTINEL_ORACLE_ADDRESS,
	feeToken: cfg.SENTINEL_FEE_TOKEN,
	consensus: cfg.CONSENSUS_ADDRESS,
	bondAmount: cfg.SENTINEL_BOND_AMOUNT,
	chainId: BigInt(cfg.CHAIN_ID),
	votingWindow: cfg.SENTINEL_VOTING_WINDOW,
} as const;

const service = new SentinelService({
	account,
	publicClient,
	config: sentinelConfig,
	detector: createDetector(cfg.SENTINEL_BLOCKLIST),
	logger,
	watcherConfig,
	database: db,
});

const shutdown = async () => {
	logger.notice("Shutting down sentinel service...");
	await Promise.all([service.stop(), metrics.stop()]);
	process.exit(0);
};
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

Promise.all([service.start(), metrics.start()]).catch((error: unknown) => {
	logger.error("Sentinel service failed to start.", { error: formatError(error) });
	process.exit(1);
});

export default {};
