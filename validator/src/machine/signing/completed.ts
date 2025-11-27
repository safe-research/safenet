import { signedEventSchema } from "../../consensus/schemas.js";
import type { MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleSigningCompleted = async (
	machineConfig: MachineConfig,
	machineStates: MachineStates,
	block: bigint,
	eventArgs: unknown,
): Promise<StateDiff> => {
	// The message was completely signed
	// Parse event from raw data
	const event = signedEventSchema.parse(eventArgs);
	// Check that state for signature id is "collect_signing_shares"
	const status = machineStates.signing.get(event.sid);
	if (status?.id !== "collect_signing_shares") return {};
	if (status.lastSigner === undefined) throw Error("Invalid state");

	return {
		signing: [
			event.sid,
			{
				id: "waiting_for_attestation",
				deadline: block + machineConfig.signingTimeout,
				responsible: status.lastSigner,
			},
		],
	};
};
