import type { TransactionAttestedEvent } from "../transitions/types.js";
import type { MachineStates, StateDiff } from "../types.js";

export const handleTransactionAttested = async (
	machineStates: MachineStates,
	event: TransactionAttestedEvent,
): Promise<StateDiff> => {
	// Check that signing state is waiting for attestation
	const status = machineStates.signing[event.message];
	if (status?.id !== "waiting_for_attestation") return {};

	// Clean up internal state
	return {
		consensus: {
			signatureIdToMessage: [status.signatureId],
		},
		signing: [event.message],
	};
};
