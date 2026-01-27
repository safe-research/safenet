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
	keyGenTimeout?: bigint;
	signingTimeout?: bigint;
	blocksBeforeResubmit?: bigint;
}

export type AbiPoint = { x: bigint; y: bigint };
