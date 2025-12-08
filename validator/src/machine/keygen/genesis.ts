import { maxUint64, zeroAddress } from "viem";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { KeyGenEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";
import { calcGenesisGroupId } from "./group.js";
import { triggerKeyGen } from "./trigger.js";

export const handleGenesisKeyGen = (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	transition: KeyGenEvent,
	logger?: (msg: unknown) => void,
): StateDiff => {
	const genesisGroupId = calcGenesisGroupId(machineConfig);
	logger?.(`Genesis group id: ${genesisGroupId}`);
	if (
		machineStates.rollover.id === "waiting_for_rollover" &&
		consensusState.activeEpoch === 0n &&
		consensusState.stagedEpoch === 0n &&
		transition.gid === genesisGroupId
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
		return {
			...diff,
			consensus,
		};
	}
	return {};
};
