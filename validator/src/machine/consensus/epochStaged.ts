import { epochStagedEventSchema } from "../../consensus/schemas.js";
import type { ConsensusState, MachineStates, StateDiff } from "../types.js";

export const handleEpochStaged = async (
	consensusState: ConsensusState,
	machineStates: MachineStates,
	eventArgs: unknown,
): Promise<StateDiff> => {
	// An epoch was staged
	const event = epochStagedEventSchema.parse(eventArgs);
	consensusState.stagedEpoch = event.proposedEpoch;
	// Ignore if not in "request_rollover_data" state
	if (machineStates.rollover.id !== "sign_rollover") {
		throw Error(
			`Not expecting epoch staging during ${machineStates.rollover.id}!`,
		);
	}
	// Check that state for signature id is "collect_signing_shares"
	const status = machineStates.signing.get(machineStates.rollover.message);
	if (status?.id !== "waiting_for_attestation") return {};

	// Clean up internal state
	return {
		consensus: { signatureIdToMessage: [status.signatureId] },
		signing: [machineStates.rollover.message],
		rollover: { id: "waiting_for_rollover" },
	};
};
