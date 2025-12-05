import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { KeyGenConfirmedEvent } from "../transitions/types.js";
import type { ConsensusDiff, ConsensusState, MachineStates, StateDiff } from "../types.js";

export const handleKeyGenConfirmed = async (
	keyGenClient: KeyGenClient,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	event: KeyGenConfirmedEvent,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	// A participant has confirmed their participation in the key gen ceremony
	// Ignore if not in "collecting_confirmations" state
	if (machineStates.rollover.id !== "collecting_confirmations") {
		logger?.(`Unexpected state ${machineStates.rollover.id}`);
		return {};
	}
	// Verify that the group corresponds to the expected group
	if (machineStates.rollover.groupId !== event.gid) {
		logger?.(`Unexpected groupId ${event.gid}`);
		return {};
	}
	const groupId = event.gid;

	// Check if this is our own confirmation
	const ourParticipantId = keyGenClient.participantId(groupId);
	if (event.identifier !== ourParticipantId) {
		// Not our confirmation, just track
		logger?.(`Group ${event.gid} confirmation from ${event.identifier} (not ours)`);
		return {
			rollover: {
				...machineStates.rollover,
				lastParticipant: event.identifier,
			},
		};
	}

	// This is our confirmation
	logger?.(`Group ${event.gid} our confirmation received`);

	// For genesis group: immediately start preprocessing after our confirmation
	// For non-genesis: the callback will trigger proposeEpoch, which triggers Sign
	// In both cases, we can start our preprocessing now
	const consensus: ConsensusDiff = {
		groupPendingNonces: [groupId, true],
	};
	const nonceTreeRoot = signingClient.generateNonceTree(groupId);
	const actions: ProtocolAction[] = [
		{
			id: "sign_register_nonce_commitments",
			groupId,
			nonceCommitmentsHash: nonceTreeRoot,
		},
	];

	if (consensusState.genesisGroupId === groupId) {
		// Genesis group: we're done with keygen, just start preprocessing
		logger?.("Genesis group confirmation done, starting preprocessing");
		return { consensus, rollover: { id: "waiting_for_rollover" }, actions };
	}

	// For non-genesis: The callback on the last confirmation will trigger proposeEpoch
	// which emits EpochProposed and then Sign event. We transition to a state that
	// can handle the incoming Sign event for the rollover message.
	logger?.(`Non-genesis group ${groupId} confirmation done, waiting for EpochProposed/Sign`);
	return {
		consensus,
		rollover: {
			id: "sign_rollover",
			groupId,
			nextEpoch: machineStates.rollover.nextEpoch,
			message: "0x" as `0x${string}`, // Will be filled by Sign event
			responsible: event.identifier,
		},
		actions,
	};
};
