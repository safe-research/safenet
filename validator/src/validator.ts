import dotenv from "dotenv";
import { privateKeyToAccount } from "viem/accounts";
import { z } from "zod";
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
const rpcUrl = validatorConfig.RPC_URL;

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

const account = privateKeyToAccount(validatorConfig.PRIVATE_KEY);
logger.info(`Using validator account ${account.address}`);

const metrics = createMetricsService({ logger, port: validatorConfig.METRICS_PORT });
const service = createValidatorService(account, rpcUrl, config, logger, metrics.metrics);

// Handle graceful shutdown, for both `SIGINT` (i.e. Ctrl-C) and `SIGTERM` which
// gets send when stopping a container or `kill`.
const shutdown = async () => {
	logger.info("Shutting down service...");
	service.stop();
	await metrics.stop();
	process.exit(0);
};
process.on("SIGINT", shutdown);
process.on("SIGTERM", shutdown);

Promise.all([service.start(), metrics.start()]).catch((error: unknown) => {
	logger.error("Service failed to start:");
	logger.error(error);
	process.exit(1);
});

export default {};
