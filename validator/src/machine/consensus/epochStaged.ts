import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { EpochStagedEvent } from "../transitions/types.js";
import type { MachineStates, StateDiff } from "../types.js";

export const handleEpochStaged = async (
	signingClient: SigningClient,
	machineStates: MachineStates,
	event: EpochStagedEvent,
): Promise<StateDiff> => {
	// An epoch was staged
	// Ignore if not in "sign_rollover" state
	if (machineStates.rollover.id !== "sign_rollover") {
		throw new Error(`Not expecting epoch staging during ${machineStates.rollover.id}!`);
	}
	// Check that signing state is waiting for attestation
	const status = machineStates.signing[machineStates.rollover.message];
	if (status?.id !== "waiting_for_attestation") return {};

	const groupId = machineStates.rollover.groupId;

	// Start preprocessing for the new group (per spec's epoch_staged handler)
	const nonceTreeRoot = signingClient.generateNonceTree(groupId);
	const actions: ProtocolAction[] = [
		{
			id: "sign_register_nonce_commitments",
			groupId,
			nonceCommitmentsHash: nonceTreeRoot,
		},
	];

	// Clean up internal state and mark group as ready for signing
	return {
		consensus: {
			stagedEpoch: event.proposedEpoch,
			signatureIdToMessage: [status.signatureId],
			groupPendingNonces: [groupId, true],
		},
		signing: [machineStates.rollover.message],
		rollover: { id: "waiting_for_rollover" },
		actions,
	};
};
