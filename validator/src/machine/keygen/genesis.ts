import { maxUint64, zeroAddress } from "viem";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";
import { triggerKeyGen } from "./trigger.js";

export const checkGenesis = (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	logger?: (msg: unknown) => void,
): StateDiff => {
	if (
		machineStates.rollover.id === "waiting_for_rollover" &&
		consensusState.activeEpoch === 0n &&
		consensusState.stagedEpoch === 0n
	) {
		logger?.("Trigger Genesis Group Generation");
		// We set no timeout for the genesis group generation
		const { groupId, diff } = triggerKeyGen(
			keyGenClient,
			0n,
			maxUint64,
			machineConfig.defaultParticipants,
			zeroAddress,
			logger,
		);
		const consensus = diff.consensus ?? {};
		consensus.genesisGroupId = groupId;
		logger?.(`Genesis group id: ${groupId}`);
		return {
			...diff,
			consensus,
		};
	}
	return {};
};
