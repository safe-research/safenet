import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { Logger } from "../../utils/logging.js";
import { calcGroupContext } from "../keygen/group.js";
import { triggerKeyGen } from "../keygen/trigger.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const checkEpochRollover = (
	machineConfig: MachineConfig,
	protocol: SafenetProtocol,
	keyGenClient: KeyGenClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	logger?: Logger,
): StateDiff => {
	const currentEpoch = block / machineConfig.blocksPerEpoch;
	const currentState = machineStates.rollover;
	if (currentState.id === "waiting_for_genesis") {
		// No automatic epoch rollover when in genesis state
		return {};
	}

	if (currentState.id === "skip_genesis") {
		// The state of the current key gen (for the next epoch) is unknown.
		// To avoid unexpected behavior, skip the current key gen and wait for the next.
		return {
			rollover: { id: "epoch_skipped", nextEpoch: currentEpoch + 1n },
		};
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
		logger?.info?.(`Rollover to epoch ${currentState.nextEpoch}`);
		const cleanupDiff = cleanupOldEpochGroups(
			keyGenClient,
			consensusState,
			machineStates,
			currentState.nextEpoch,
			logger,
		);
		rolloverDiff.consensus = {
			activeEpoch: currentState.nextEpoch,
			...cleanupDiff.consensus,
		};
	}

	// Trigger key gen for next epoch
	const nextEpoch = currentEpoch + 1n;
	logger?.info?.(`Trigger key gen for epoch ${nextEpoch}`);
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

/**
 * Remove old epoch groups that are no longer needed.
 *
 * Strategy:
 * 1. Find the smallest epoch referenced by any active signing session
 * 2. Find the previous epoch (largest epoch key < activatingEpoch) from epochGroups
 * 3. Threshold = min(smallestSigningEpoch, previousEpoch)
 * 4. Remove all epoch groups with epoch < threshold, and unregister their FROST groups
 *
 * This ensures:
 * - The activating epoch and all future epochs are always preserved
 * - The previous epoch is always preserved (for late attestations)
 * - Any epoch still referenced by an active signing session is preserved
 */
const cleanupOldEpochGroups = (
	keyGenClient: KeyGenClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	activatingEpoch: bigint,
	logger?: Logger,
): StateDiff => {
	const epochKeys = Object.keys(consensusState.epochGroups).map(BigInt);
	if (epochKeys.length === 0) return {};

	// Find the previous epoch: the largest epoch key strictly less than the activating epoch
	let previousEpoch: bigint | undefined;
	for (const epoch of epochKeys) {
		if (epoch < activatingEpoch) {
			if (previousEpoch === undefined || epoch > previousEpoch) {
				previousEpoch = epoch;
			}
		}
	}

	// No previous epochs to clean up
	if (previousEpoch === undefined) return {};

	// Find the smallest epoch referenced by any active signing session
	let smallestSigningEpoch: bigint | undefined;
	for (const status of Object.values(machineStates.signing)) {
		const epoch =
			status.packet.type === "epoch_rollover_packet"
				? status.packet.rollover.activeEpoch
				: status.packet.proposal.epoch;
		if (smallestSigningEpoch === undefined || epoch < smallestSigningEpoch) {
			smallestSigningEpoch = epoch;
		}
	}

	// Threshold: remove epoch groups strictly below this value
	const threshold =
		smallestSigningEpoch !== undefined && smallestSigningEpoch < previousEpoch ? smallestSigningEpoch : previousEpoch;

	// Check if there is anything to remove
	const hasEpochsBelowThreshold = epochKeys.some((epoch) => epoch < threshold);
	if (!hasEpochsBelowThreshold) return {};

	// Unregister FROST groups for removed epochs (cascading deletes clean up all related data)
	for (const epoch of epochKeys) {
		if (epoch < threshold) {
			const groupInfo = consensusState.epochGroups[epoch.toString()];
			if (groupInfo !== undefined) {
				logger?.info?.(`Cleaning up epoch group for epoch ${epoch} (group ${groupInfo.groupId})`);
				keyGenClient.unregisterGroup(groupInfo.groupId);
			}
		}
	}

	return {
		consensus: {
			removeEpochGroupsBefore: threshold,
		},
	};
};
