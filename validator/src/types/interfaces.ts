import type { Address, Hex } from "viem";
import type { SupportedChain } from "./schemas.js";

export type ParticipantInfo = {
	address: Address;
	activeFrom: bigint;
	activeBefore?: bigint;
};

export interface ProtocolConfig {
	chainId: SupportedChain;
	consensus: Address;
	coordinator: Address;
	staker: Address;
	blocksPerEpoch: bigint;
	participants: ParticipantInfo[];
	genesisSalt: Hex;
	keyGenTimeout?: bigint;
	signingTimeout?: bigint;
	blocksBeforeResubmit?: bigint;
	allowedOracles?: readonly Address[];
	oracleTimeout?: bigint;
}

export type AbiPoint = { x: bigint; y: bigint };
