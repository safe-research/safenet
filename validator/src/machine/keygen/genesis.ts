import { maxUint64 } from "viem";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { Logger } from "../../utils/logging.js";
import { participantsForEpoch } from "../../utils/participants.js";
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
	logger?: Logger,
): Promise<StateDiff> => {
	const genesisGroup = calcGenesisGroup(machineConfig);
	if (
		machineStates.rollover.id === "waiting_for_genesis" &&
		consensusState.activeEpoch === 0n &&
		transition.gid === genesisGroup.id
	) {
		logger?.info?.("Trigger Genesis Group Generation", { genesisGroup });
		// Set no timeout for the genesis group generation
		const diff = triggerKeyGen(
			machineConfig,
			keyGenClient,
			0n,
			maxUint64,
			participantsForEpoch(machineConfig.participantsInfo, 0n),
			genesisGroup.context,
			logger,
		);
		const rollover = diff.rollover;
		if (rollover?.id !== "collecting_commitments") {
			throw new Error(`Unexpected genesis rollover state ${rollover?.id}`);
		}
		if (rollover?.nextEpoch !== 0n || rollover?.groupId !== genesisGroup.id) {
			throw new Error(`Unexpected genesis group ${rollover?.groupId}`);
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
