import type { SignedEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleSigningCompleted = async (
	machineConfig: MachineConfig,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	event: SignedEvent,
): Promise<StateDiff> => {
	// Check that this is a request related to a message that is handled"
	const message = consensusState.signatureIdToMessage[event.sid];
	if (message === undefined) return {};
	// Check that state for signature id is "collect_signing_shares"
	const status = machineStates.signing[message];
	if (status?.id !== "collect_signing_shares") return {};
	// If signing shares where collected (based on previous state check),
	// then it is a logic error that there is no last signer,
	if (status.lastSigner === undefined) throw new Error("Invalid state");

	return {
		signing: [
			message,
			{
				id: "waiting_for_attestation",
				signatureId: status.signatureId,
				deadline: event.block + machineConfig.signingTimeout,
				responsible: status.lastSigner,
				packet: status.packet,
			},
		],
	};
};
