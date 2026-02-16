import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { EpochStagedEvent } from "../transitions/types.js";
import type { ConsensusDiff, MachineStates, StateDiff } from "../types.js";

export const handleEpochStaged = async (
	signingClient: SigningClient,
	machineStates: MachineStates,
	event: EpochStagedEvent,
): Promise<StateDiff> => {
	// An epoch was staged
	// Ignore if not in "sign_rollover" state
	if (machineStates.rollover.id !== "sign_rollover") {
		return {};
	}

	const signatureIdDiff: ConsensusDiff = {};
	// Check if there is a signatureId that needs to be cleaned up
	const status = machineStates.signing[machineStates.rollover.message];
	if (status !== undefined && status.id !== "waiting_for_request") {
		signatureIdDiff.signatureIdToMessage = [status.signatureId, undefined];
	}
	const groupId = machineStates.rollover.groupId;
	// The signing state should be cleaned up in any case, as the rollover was attested
	const diff: StateDiff = {
		consensus: {
			...signatureIdDiff,
		},
		rollover: { id: "epoch_staged", nextEpoch: event.proposedEpoch },
		signing: [machineStates.rollover.message, undefined],
	};

	try {
		// Check if validator is part of group, method will throw if not
		signingClient.participantId(groupId);
	} catch {
		// If there is no participant id, then this validator is not part of the group
		// In this case don't generate a nonce tree
		return diff;
	}

	// Start preprocessing for the new group (per spec's epoch_staged handler)
	const nonceTreeRoot = signingClient.generateNonceTree(groupId);
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
