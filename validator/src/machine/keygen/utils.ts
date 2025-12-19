import { encodeAbiParameters, type Hex } from "viem";
import type { MachineConfig } from "../types.js";

export const buildKeyGenCallback = (machineConfig: MachineConfig, nextEpoch: bigint): Hex | undefined => {
	if (nextEpoch === 0n) {
		// Don't build a callback for the genesis group
		return undefined;
	}
	// For non-genesis groups, we include callback context to trigger epoch proposal
	const rolloverBlock = nextEpoch * machineConfig.blocksPerEpoch;
	// ABI encode: (uint64 proposedEpoch, uint64 rolloverBlock)
	return encodeAbiParameters([{ type: "uint64" }, { type: "uint64" }], [nextEpoch, rolloverBlock]);
};
