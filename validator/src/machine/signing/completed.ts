import { signedEventSchema } from "../../consensus/schemas.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleSigningCompleted = async (
	machineConfig: MachineConfig,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	eventArgs: unknown,
): Promise<StateDiff> => {
	// The message was completely signed
	// Parse event from raw data
	const event = signedEventSchema.parse(eventArgs);
	// Check that this is a request related to a message that is handled"
	const message = consensusState.signatureIdToMessage[event.sid];
	if (message === undefined) return {};
	// Check that state for signature id is "collect_signing_shares"
	const status = machineStates.signing[message];
	if (status?.id !== "collect_signing_shares") return {};
	if (status.lastSigner === undefined) throw new Error("Invalid state");

	return {
		signing: [
			message,
			{
				id: "waiting_for_attestation",
				signatureId: status.signatureId,
				deadline: block + machineConfig.signingTimeout,
				responsible: status.lastSigner,
				packet: status.packet,
			},
		],
	};
};
