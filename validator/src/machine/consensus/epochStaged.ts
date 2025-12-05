import type { EpochStagedEvent } from "../transitions/types.js";
import type { MachineStates, StateDiff } from "../types.js";

export const handleEpochStaged = async (machineStates: MachineStates, event: EpochStagedEvent): Promise<StateDiff> => {
	// Ignore if not in "request_rollover_data" state
	if (machineStates.rollover.id !== "sign_rollover") {
		throw new Error(`Not expecting epoch staging during ${machineStates.rollover.id}!`);
	}
	// Check that signing state is waiting for attestation
	const status = machineStates.signing[machineStates.rollover.message];
	if (status?.id !== "waiting_for_attestation") return {};

	// Clean up internal state
	return {
		consensus: {
			stagedEpoch: event.proposedEpoch,
			signatureIdToMessage: [status.signatureId],
		},
		signing: [machineStates.rollover.message],
		rollover: { id: "waiting_for_rollover" },
	};
};
