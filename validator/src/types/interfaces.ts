import type { Address, Hex } from "viem";
import type { Participant } from "../consensus/storage/types.js";
import type { SupportedChain } from "./schemas.js";

export interface ProtocolConfig {
	chainId: SupportedChain;
	consensus: Address;
	coordinator: Address;
	blocksPerEpoch: bigint;
	participants: Participant[];
	genesisSalt: Hex;
}

export type AbiPoint = { x: bigint; y: bigint };
