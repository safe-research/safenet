import { maxUint64 } from "viem";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { KeyGenEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";
import { calcGenesisGroup } from "./group.js";
import { triggerKeyGen } from "./trigger.js";

export const handleGenesisKeyGen = async (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	transition: KeyGenEvent,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	const genesisGroup = calcGenesisGroup(machineConfig);
	logger?.(`Genesis group id: ${genesisGroup.id}`);
	if (
		machineStates.rollover.id === "waiting_for_rollover" &&
		consensusState.activeEpoch === 0n &&
		consensusState.stagedEpoch === 0n &&
		transition.gid === genesisGroup.id
	) {
		logger?.("Trigger Genesis Group Generation");
		// Set no timeout for the genesis group generation
		const diff = triggerKeyGen(
			machineConfig,
			keyGenClient,
			0n,
			maxUint64,
			machineConfig.defaultParticipants,
			genesisGroup.context,
			logger,
		);
		const epochGroup = diff.consensus?.epochGroup;
		if (epochGroup === undefined || epochGroup[0] !== 0n || epochGroup[1].groupId !== genesisGroup.id) {
			throw new Error(`Unexpected genesis group ${epochGroup?.[1].groupId}`);
		}
		const consensus = diff.consensus ?? {};
		consensus.genesisGroupId = genesisGroup.id;
		return {
			...diff,
			consensus,
		};
	}
	return {};
};
