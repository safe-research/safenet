import { transactionAttestedEventSchema } from "../../consensus/schemas.js";
import type { MachineStates, StateDiff } from "../types.js";

export const handleTransactionAttested = async (
	machineStates: MachineStates,
	eventArgs: unknown,
): Promise<StateDiff> => {
	// The transaction attestation was submitted
	// Parse event from raw data
	const event = transactionAttestedEventSchema.parse(eventArgs);
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
