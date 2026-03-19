import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { EpochStagedEvent } from "../transitions/types.js";
import type { ConsensusDiff, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleEpochStaged = async (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	machineStates: MachineStates,
	event: EpochStagedEvent,
): Promise<StateDiff> => {
	// An epoch was staged
	// Ignore if not in "sign_rollover" state
	if (machineStates.rollover.id !== "sign_rollover") {
		return {};
	}

	const consensus: ConsensusDiff = {};
	// Check if there is a signatureId that needs to be cleaned up
	const status = machineStates.signing[machineStates.rollover.message];
	if (status !== undefined && status.id !== "waiting_for_request") {
		consensus.signatureIdToMessage = [status.signatureId, undefined];
	}
	const groupId = machineStates.rollover.groupId;
	// The signing state should be cleaned up in any case, as the rollover was attested
	const diff: StateDiff = {
		consensus,
		rollover: { id: "epoch_staged", nextEpoch: event.proposedEpoch },
		signing: [machineStates.rollover.message, undefined],
	};

	if (!signingClient.hasParticipant(groupId, machineConfig.account)) {
		return diff;
	}

	// Start preprocessing for the new group (per spec's epoch_staged handler)
	const nonceTreeRoot = signingClient.generateNonceTree(groupId, machineConfig.account);
	const actions: ProtocolAction[] = [
		{
			id: "sign_register_nonce_commitments",
			groupId,
			nonceCommitmentsHash: nonceTreeRoot,
		},
	];
	diff.consensus = {
		...diff.consensus,
		groupPendingNonces: [groupId, true],
	};

	// Clean up internal state and mark group as ready for signing
	return {
		...diff,
		actions,
	};
};
