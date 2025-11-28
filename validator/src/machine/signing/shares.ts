import { signatureShareEventSchema } from "../../consensus/schemas.js";
import type { ConsensusState, MachineStates, StateDiff } from "../types.js";

export const handleSigningShares = async (
	consensusState: ConsensusState,
	machineStates: MachineStates,
	eventArgs: unknown,
): Promise<StateDiff> => {
	// A participant has submitted a singature share for a signature id
	// Parse event from raw data
	const event = signatureShareEventSchema.parse(eventArgs);
	// Check that this is a request related to a message that is handled"
	const message = consensusState.signatureIdToMessage[event.sid];
	if (message === undefined) return {};
	const status = machineStates.signing[message];
	// Check that state for signature id is "collect_signing_shares"
	if (status?.id !== "collect_signing_shares") return {};
	// Track identity that has submitted last share
	status.sharesFrom.push(event.identifier);
	status.lastSigner = event.identifier;
	return {
		signing: [event.sid, status],
	};
};
