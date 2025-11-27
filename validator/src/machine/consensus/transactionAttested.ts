import { transactionAttestedEventSchema } from "../../consensus/schemas.js";
import type { ConsensusState, MachineStates, StateDiff } from "../types.js";

export const handleTransactionAttested = async (
	machineStates: MachineStates,
	consensusState: ConsensusState,
	eventArgs: unknown,
): Promise<StateDiff> => {
	// The transaction attestation was submitted
	// Parse event from raw data
	const event = transactionAttestedEventSchema.parse(eventArgs);
	// Get current signature id for message
	const signatureRequest = consensusState.messageSignatureRequests.get(
		event.message,
	);
	if (signatureRequest === undefined) return {};
	// Check that state for signature id is "collect_signing_shares"
	const status = machineStates.signing.get(signatureRequest);
	if (status?.id !== "waiting_for_attestation") return {};

	// Clean up internal state
	// TODO: refactor to state diff
	consensusState.messageSignatureRequests.delete(event.message);
	consensusState.transactionProposalInfo.delete(event.message);
	return {
		signing: [signatureRequest, undefined],
	};
};
