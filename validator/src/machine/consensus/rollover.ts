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
		const cleanupDiff = computeCleanupThreshold(consensusState, machineStates);
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
 * Computes the cleanup cutoff for old epoch groups.
 *
 * Returns a `removeEpochGroupsBefore` value in the diff, which is later used to:
 * - Remove epoch group entries from the machine state (in `applyConsensus`)
 * - Unregister the corresponding FROST groups from the crypto DB (in `SafenetStateMachine`)
 *
 * Strategy:
 * 1. Start with the currently active epoch as the cutoff (always preserved for late attestations)
 * 2. Narrow the cutoff to the smallest epoch referenced by any active signing session, if lower
 *
 * This ensures:
 * - The active epoch and all future epochs are always preserved
 * - Any epoch still referenced by an active signing session is preserved
 */
const computeCleanupThreshold = (
	consensusState: ConsensusState,
	machineStates: MachineStates,
): Pick<StateDiff, "consensus"> => {
	// Preserve the currently active epoch; narrow down if a signing session references an older epoch.
	let epochCutoff = consensusState.activeEpoch;
	for (const status of Object.values(machineStates.signing)) {
		const epoch =
			status.packet.type === "epoch_rollover_packet"
				? // Rollover packets are signed with the active epoch's key; proposedEpoch is the new epoch being set up.
					status.packet.rollover.activeEpoch
				: status.packet.proposal.epoch;
		if (epoch < epochCutoff) {
			epochCutoff = epoch;
		}
	}

	// Check if there is anything to remove.
	const hasEpochsBelowCutoff = Object.keys(consensusState.epochGroups).some((key) => BigInt(key) < epochCutoff);
	if (!hasEpochsBelowCutoff) return {};

	return {
		consensus: {
			removeEpochGroupsBefore: epochCutoff,
		},
	};
};
