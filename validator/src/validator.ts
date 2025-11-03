import dotenv from "dotenv";
import { z } from "zod";
import { createValidatorService } from "./service/service.js";
import type { ConsensusConfig } from "./types/interfaces.js";
import { validatorConfigSchema } from "./types/schemas.js";

dotenv.config();

const result = validatorConfigSchema.safeParse(process.env);
if (!result.success) {
	console.error(
		"Invalid environment variable configuration:",
		z.treeifyError(result.error),
	);
	process.exit(1);
}

const validatorConfig = result.data;
const rpcUrl = validatorConfig.RPC_URL;
const config: ConsensusConfig = {
	coreAddress: validatorConfig.CONSENSUS_CORE_ADDRESS,
};

const service = createValidatorService(rpcUrl, config);

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
