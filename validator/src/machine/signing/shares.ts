import type { SignatureShareEvent } from "../transitions/types.js";
import type { ConsensusState, MachineStates, StateDiff } from "../types.js";

export const handleSigningShares = async (
	consensusState: ConsensusState,
	machineStates: MachineStates,
	event: SignatureShareEvent,
): Promise<StateDiff> => {
	// Check that this is a request related to a message that is handled"
	const message = consensusState.signatureIdToMessage[event.sid];
	if (message === undefined) return {};
	const status = machineStates.signing[message];
	// Check that state for signature id is "collect_signing_shares"
	if (status?.id !== "collect_signing_shares") return {};
	// Track identity that has submitted last share
	// Copy all elements to avoid mutating the original array
	const sharesFrom = [...status.sharesFrom, event.identifier];
	return {
		signing: [
			message,
			{
				...status,
				lastSigner: event.identifier,
				sharesFrom,
			},
		],
	};
};
