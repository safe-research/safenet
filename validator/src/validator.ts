import dotenv from "dotenv";
import { createValidatorService } from "./service/service.js";
import type { ConsensusConfig } from "./types/interfaces.js";
import { validatorConfigSchema } from "./types/schemas.js";

dotenv.config();

const validatorConfig = validatorConfigSchema.parse(process.env);

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
