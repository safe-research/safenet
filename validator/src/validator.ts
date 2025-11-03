import dotenv from "dotenv";
import type { Address } from "viem";
import { createValidatorService } from "./service/service.js";
import type { ConsensusConfig } from "./types/interfaces.js";

dotenv.config();

// TODO use zod to validate input
const rpcUrl = process.env.RPC_URL!;

const config: ConsensusConfig = {
	coreAddress: process.env.CONSENSUS_CORE_ADDRESS! as Address,
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
