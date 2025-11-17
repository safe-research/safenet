import type { Address } from "viem";

export interface ConsensusConfig {
	coreAddress: Address;
}

export type AbiPoint = { x: bigint; y: bigint };
