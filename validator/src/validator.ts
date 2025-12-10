import dotenv from "dotenv";
import { privateKeyToAccount } from "viem/accounts";
import { z } from "zod";
import { createValidatorService } from "./service/service.js";
import type { ProtocolConfig } from "./types/interfaces.js";
import { validatorConfigSchema } from "./types/schemas.js";

dotenv.config({ quiet: true });

const BLOCKTIME_IN_SECONDS = 5n; // value assumed for gnosis chain
const BLOCKS_PER_EPOCH = (5n * 60n) / BLOCKTIME_IN_SECONDS; // ~ blocks for 5 minutes

const result = validatorConfigSchema.safeParse(process.env);
if (!result.success) {
	console.log(result.error);
	console.error("Invalid environment variable configuration:", z.treeifyError(result.error));
	process.exit(1);
}

const validatorConfig = result.data;
const rpcUrl = validatorConfig.RPC_URL;

const config: ProtocolConfig = {
	chainId: validatorConfig.CHAIN_ID,
	consensus: validatorConfig.CONSENSUS_ADDRESS,
	coordinator: validatorConfig.COORDINATOR_ADDRESS,
	participants: validatorConfig.PARTICIPANTS,
	genesisSalt: validatorConfig.GENESIS_SALT,
	blocksPerEpoch: BLOCKS_PER_EPOCH,
};

const account = privateKeyToAccount(validatorConfig.PRIVATE_KEY);
console.log(`Using validator account ${account.address}`);

const service = createValidatorService(account, rpcUrl, config);

// Handle graceful shutdown
process.on("SIGINT", () => {
	console.log("Shutting down service...");
	service.stop();
	process.exit(0);
});

service.start().catch((error: unknown) => {
	console.error("Service failed to start:");
	console.error(error);
	process.exit(1);
});

export default {};
