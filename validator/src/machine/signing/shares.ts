import { signatureShareEventSchema } from "../../consensus/schemas.js";
import type { MachineStates, StateDiff } from "../types.js";

export const handleSigningShares = async (
	machineStates: MachineStates,
	eventArgs: unknown,
): Promise<StateDiff> => {
	// A participant has submitted a singature share for a signature id
	// Parse event from raw data
	const event = signatureShareEventSchema.parse(eventArgs);
	// Check that state for signature id is "collect_signing_shares"
	const status = machineStates.signing.get(event.sid);
	if (status?.id !== "collect_signing_shares") return {};
	// Track identity that has submitted last share
	status.sharesFrom.push(event.identifier);
	status.lastSigner = event.identifier;
	return {
		signing: [event.sid, status],
	};
};
