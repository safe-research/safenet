import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const checkKeyGenAbort = (
	machineConfig: MachineConfig,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	logger?: (msg: unknown) => void,
): StateDiff => {
	if (
		machineStates.rollover.id === "waiting_for_rollover" ||
		machineStates.rollover.groupId === consensusState.genesisGroupId
	) {
		return {};
	}
	const currentEpoch = block / machineConfig.blocksPerEpoch;
	if (currentEpoch < machineStates.rollover.nextEpoch) {
		// Still valid epoch
		return {};
	}
	logger?.(`Abort keygen for ${machineStates.rollover.nextEpoch}`);
	return {
		rollover: { id: "waiting_for_rollover" },
	};
};
