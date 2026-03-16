import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ComplaintResponse, SafenetProtocol } from "../../consensus/protocol/types.js";
import type { Logger } from "../../utils/logging.js";
import type { KeyGenComplaintSubmittedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates, RolloverState, StateDiff } from "../types.js";
import { calcGroupContext } from "./group.js";
import { triggerKeyGen } from "./trigger.js";

export const handleComplaintSubmitted = async (
	machineConfig: MachineConfig,
	protocol: SafenetProtocol,
	keyGenClient: KeyGenClient,
	machineStates: MachineStates,
	event: KeyGenComplaintSubmittedEvent,
	logger?: Logger,
): Promise<StateDiff> => {
	if (machineStates.rollover.id !== "collecting_shares" && machineStates.rollover.id !== "collecting_confirmations") {
		return {};
	}

	if (machineStates.rollover.groupId !== event.gid) {
		return {};
	}

	if (
		machineStates.rollover.id === "collecting_confirmations" &&
		event.block > machineStates.rollover.complaintDeadline
	) {
		return {};
	}

	// Get or create complaints entry for accused
	const complaint = machineStates.rollover.complaints[event.accused] ?? { total: 0, unresponded: 0 };
	// Copy complaints with update
	const nextComplaint = {
		total: complaint.total + 1,
		unresponded: complaint.unresponded + 1,
	};

	const threshold = keyGenClient.threshold(event.gid);
	if (nextComplaint.total >= threshold) {
		const participants = keyGenClient.participants(event.gid);
		const nextParticipants = participants.filter((p) => p !== event.accused);
		logger?.info?.(`Restarting key gen after too many complaints against participant ${event.accused}`);
		return triggerKeyGen(
			machineConfig,
			keyGenClient,
			machineStates.rollover.nextEpoch,
			event.block + machineConfig.keyGenTimeout,
			nextParticipants,
			calcGroupContext(protocol.consensus(), machineStates.rollover.nextEpoch),
			logger,
		);
	}

	const rollover: RolloverState = {
		...machineStates.rollover,
		complaints: {
			...machineStates.rollover.complaints,
			[event.accused]: nextComplaint,
		},
	};

	if (event.accused !== keyGenClient.participant(event.gid)) {
		return {
			rollover,
		};
	}
	// We are accused, reveal share for plaintiff
	const action: ComplaintResponse = {
		id: "key_gen_complaint_response",
		groupId: event.gid,
		plaintiff: event.plaintiff,
		secretShare: keyGenClient.createSecretShare(event.gid, event.plaintiff),
	};
	return {
		rollover,
		actions: [action],
	};
};
