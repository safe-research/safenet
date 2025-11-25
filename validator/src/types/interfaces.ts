import type { Address } from "viem";
import type { Participant } from "../consensus/storage/types.js";
import type { SupportedChain } from "./schemas.js";

export interface ProtocolConfig {
	chainId: SupportedChain;
	conensus: Address;
	coordinator: Address;
	blocksPerEpoch: bigint;
	participants: Participant[];
}

export type AbiPoint = { x: bigint; y: bigint };
