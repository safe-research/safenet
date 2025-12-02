import type { MutableConsensusState, MutableMachineStates, Optional, StateDiff } from "../types.js";

export type UpdatableConsensusState = Optional<MutableConsensusState, "activeEpoch" | "stagedEpoch" | "genesisGroupId">;
export type UpdatableMachineState = Optional<MutableMachineStates, "rollover">;

export const applyMachines = (diff: Pick<StateDiff, "rollover" | "signing">, machineStates: UpdatableMachineState) => {
	if (diff.signing !== undefined) {
		const [signatureId, state] = diff.signing;
		if (state === undefined) {
			delete machineStates.signing[signatureId];
		} else {
			machineStates.signing[signatureId] = state;
		}
	}
	if (diff.rollover !== undefined) {
		machineStates.rollover = diff.rollover;
	}
};

export const applyConsensus = (diff: Pick<StateDiff, "consensus">, consensusState: UpdatableConsensusState) => {
	if (diff.consensus !== undefined) {
		const consensusDiff = diff.consensus;
		if (consensusDiff.groupPendingNonces !== undefined) {
			const [groupId, pendingNonces] = consensusDiff.groupPendingNonces;
			if (pendingNonces === true) {
				consensusState.groupPendingNonces[groupId] = true;
			} else {
				delete consensusState.groupPendingNonces[groupId];
			}
		}
		if (consensusDiff.activeEpoch !== undefined) {
			consensusState.activeEpoch = consensusDiff.activeEpoch;
		}
		if (consensusDiff.stagedEpoch !== undefined) {
			consensusState.stagedEpoch = consensusDiff.stagedEpoch;
		}
		if (consensusDiff.genesisGroupId !== undefined) {
			consensusState.genesisGroupId = consensusDiff.genesisGroupId;
		}
		if (consensusDiff.epochGroup !== undefined) {
			const [epoch, groupInfo] = consensusDiff.epochGroup;
			consensusState.epochGroups[epoch.toString()] = groupInfo;
		}
		if (consensusDiff.signatureIdToMessage !== undefined) {
			const [signatureId, message] = consensusDiff.signatureIdToMessage;
			if (message === undefined) {
				delete consensusState.signatureIdToMessage[signatureId];
			} else {
				consensusState.signatureIdToMessage[signatureId] = message;
			}
		}
	}
};
