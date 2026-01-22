import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import { calcGroupContext } from "../keygen/group.js";
import { triggerKeyGen } from "../keygen/trigger.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const checkEpochRollover = (
	machineConfig: MachineConfig,
	protocol: ShieldnetProtocol,
	keyGenClient: KeyGenClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	logger?: (msg: unknown) => void,
): StateDiff => {
	const currentEpoch = block / machineConfig.blocksPerEpoch;
	let activeEpoch = consensusState.activeEpoch;
	let stagedEpoch = consensusState.stagedEpoch;
	if (stagedEpoch > 0n && stagedEpoch <= currentEpoch) {
		logger?.(`Update active epoch from ${consensusState.activeEpoch} to ${consensusState.stagedEpoch}`);
		// Update active epoch
		activeEpoch = consensusState.stagedEpoch;
		stagedEpoch = 0n;
	}
	// If no rollover is staged and new key gen was not triggered do it now
	if (
		machineStates.rollover.id === "waiting_for_rollover" &&
		stagedEpoch === 0n &&
		// Do not trigger a new key gen on genesis
		(activeEpoch !== 0n || consensusState.genesisGroupId !== undefined)
	) {
		// Trigger key gen for next epoch
		const nextEpoch = currentEpoch + 1n;
		logger?.(`Trigger key gen for epoch ${nextEpoch}`);
		// For each epoch rollover key gen trigger always use the default participants
		// This allows previously removed validators to recover
		const { diff } = triggerKeyGen(
			keyGenClient,
			nextEpoch,
			block + machineConfig.keyGenTimeout,
			machineConfig.defaultParticipants,
			calcGroupContext(protocol.consensus(), nextEpoch),
			logger,
		);
		const consensus = {
			...diff.consensus,
			activeEpoch,
			stagedEpoch,
		};
		return {
			...diff,
			consensus,
		};
	}
	if (activeEpoch !== consensusState.activeEpoch || stagedEpoch !== consensusState.stagedEpoch) {
		return { consensus: { activeEpoch, stagedEpoch } };
	}
	return {};
};
