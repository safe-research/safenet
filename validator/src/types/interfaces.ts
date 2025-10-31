import type { Address } from "viem";
import type z from "zod";
import type { validatorConfigSchema } from "./schemas.js";

export type ValidatorConfig = z.infer<typeof validatorConfigSchema>;

export interface ConsensusConfig {
	coreAddress: Address;
}
