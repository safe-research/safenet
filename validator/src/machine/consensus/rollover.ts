import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import { calcGroupContext } from "../keygen/group.js";
import { triggerKeyGen } from "../keygen/trigger.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const checkEpochRollover = (
	machineConfig: MachineConfig,
	protocol: ShieldnetProtocol,
	keyGenClient: KeyGenClient,
	_consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	logger?: (msg: unknown) => void,
): StateDiff => {
	const currentEpoch = block / machineConfig.blocksPerEpoch;
	const currentState = machineStates.rollover;
	if (currentState.id === "waiting_for_genesis") {
		// No automatic epoch rollover when in genesis state
		return {};
	}

	if (currentState.id !== "epoch_staged" && currentState.nextEpoch === 0n) {
		// Rollover should not happen while in genesis keygen.
		return {};
	}

	// This check applies to all states
	// When staged or skipped then keygen should be started for next epoch
	// When in one of the other state keygen should be aborted and restarted for next epoch
	if (currentState.nextEpoch > currentEpoch) {
		// Rollover should not happen yet.
		return {};
	}

	const rolloverDiff: StateDiff = {};
	if (currentState.id === "epoch_staged") {
		rolloverDiff.consensus = {
			activeEpoch: currentState.nextEpoch,
		};
	}

	// Trigger key gen for next epoch
	const nextEpoch = currentEpoch + 1n;
	logger?.(`Trigger key gen for epoch ${nextEpoch}`);
	// For each epoch rollover key gen trigger always use the default participants
	// This allows previously removed validators to recover
	const diff = triggerKeyGen(
		machineConfig,
		keyGenClient,
		nextEpoch,
		block + machineConfig.keyGenTimeout,
		machineConfig.defaultParticipants,
		calcGroupContext(protocol.consensus(), nextEpoch),
		logger,
	);
	const consensus = {
		...diff.consensus,
		...rolloverDiff.consensus,
	};
	return {
		...diff,
		consensus,
	};
};
