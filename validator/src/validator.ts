import dotenv from "dotenv";
import { privateKeyToAccount } from "viem/accounts";
import { z } from "zod";
import type { Participant } from "./consensus/storage/types.js";
import { createValidatorService } from "./service/service.js";
import type { ProtocolConfig } from "./types/interfaces.js";
import { validatorConfigSchema } from "./types/schemas.js";

dotenv.config();

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

const participants: Participant[] = validatorConfig.PARTICIPANTS.map((address, i) => {
	return {
		address,
		id: BigInt(i + 1),
	};
});

const config: ProtocolConfig = {
	chainId: validatorConfig.CHAIN_ID,
	conensus: validatorConfig.CONSENSUS_ADDRESS,
	coordinator: validatorConfig.COORDINATOR_ADDRESS,
	blocksPerEpoch: BLOCKS_PER_EPOCH,
	participants,
};

const account = privateKeyToAccount(validatorConfig.PRIVATE_KEY);

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
