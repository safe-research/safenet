import dotenv from "dotenv";
import type { ChainFees } from "viem";
import { createNonceManager, privateKeyToAccount } from "viem/accounts";
import { jsonRpc } from "viem/nonce";
import { z } from "zod";
import type { WatcherConfig } from "./machine/transitions/watcher.js";
import { createValidatorService } from "./service/service.js";
import type { ProtocolConfig } from "./types/interfaces.js";
import { validatorConfigSchema } from "./types/schemas.js";
import { createLogger } from "./utils/logging.js";
import { createMetricsService } from "./utils/metrics.js";

dotenv.config({ quiet: true });

const result = validatorConfigSchema.safeParse(process.env);
if (!result.success) {
	console.log(result.error);
	console.error("Invalid environment variable configuration:", z.treeifyError(result.error));
	process.exit(1);
}

const validatorConfig = result.data;

const logger = createLogger({
	level: validatorConfig.LOG_LEVEL,
	pretty: process.stdout.isTTY,
});

const config: ProtocolConfig = {
	chainId: validatorConfig.CHAIN_ID,
	consensus: validatorConfig.CONSENSUS_ADDRESS,
	coordinator: validatorConfig.COORDINATOR_ADDRESS,
	participants: validatorConfig.PARTICIPANTS,
	genesisSalt: validatorConfig.GENESIS_SALT,
	blocksPerEpoch: validatorConfig.BLOCKS_PER_EPOCH,
};
const watcherConfig: WatcherConfig = {
	blockTimeOverride: validatorConfig.BLOCK_TIME_OVERRIDE,
	maxReorgDepth: validatorConfig.MAX_REORG_DEPTH ?? 5,
	blockPageSize: validatorConfig.BLOCK_PAGE_SIZE,
	maxLogsPerQuery: validatorConfig.MAX_LOGS_PER_QUERY,
};
logger.notice("Using configuration", { config, watcherConfig });

const fees: ChainFees = {
	// Use a higher default multiplier to ensure transaction inclusion
	baseFeeMultiplier: validatorConfig.BASE_FEE_MULTIPLIER ?? 2,
	// Allow to set higher default priority fee to ensure transaction inclusion
	maxPriorityFeePerGas: validatorConfig.PRIORITY_FEE_PER_GAS,
};

const account = privateKeyToAccount(validatorConfig.PRIVATE_KEY, {
	nonceManager: createNonceManager({ source: jsonRpc() }),
});
logger.notice(`Using validator account ${account.address}`);

const metrics = createMetricsService({ logger, port: validatorConfig.METRICS_PORT });
const service = createValidatorService({
	account,
	rpcUrl: validatorConfig.RPC_URL,
	storageFile: validatorConfig.STORAGE_FILE,
	config,
	watcherConfig,
	logger,
	metrics: metrics.metrics,
	fees,
});

// Handle graceful shutdown, for both `SIGINT` (i.e. Ctrl-C) and `SIGTERM` which
// gets send when stopping a container or `kill`.
const shutdown = async () => {
	logger.notice("Shutting down service...");
	await Promise.all([service.stop(), metrics.stop()]);
	process.exit(0);
};
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

Promise.all([service.start(), metrics.start()]).catch((error: unknown) => {
	logger.error("Service failed to start.", { error });
	process.exit(1);
});

export default {};
