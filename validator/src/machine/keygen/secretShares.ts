import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { ParticipantId } from "../../frost/types.js";
import type { KeyGenSecretSharedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates, StateDiff } from "../types.js";
import { buildKeyGenCallback } from "./utils.js";

export const handleKeyGenSecretShared = async (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	machineStates: MachineStates,
	event: KeyGenSecretSharedEvent,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	// A participant has submitted secret share for new group
	// Ignore if not in "collecting_shares" state
	if (machineStates.rollover.id !== "collecting_shares") {
		logger?.(`Unexpected state ${machineStates.rollover.id}`);
		return {};
	}
	const groupId = event.gid;

	// Verify that the group corresponds to the next epoch
	if (machineStates.rollover.groupId !== groupId) {
		logger?.(`Unexpected groupId ${groupId}`);
		return {};
	}

	try {
		// Check if validator is part of group, method will throw if not
		keyGenClient.participantId(groupId);
	} catch {
		// If there is no participant id, then this validator is not part of the group
		// In this case ignore this request
		return {};
	}

	// Track identity that has submitted last share
	const response = await keyGenClient.handleKeygenSecrets(groupId, event.identifier, event.share.f);
	const missingSharesFrom: ParticipantId[] = [...machineStates.rollover.missingSharesFrom];
	const actions: ProtocolAction[] = [];
	if (response === "invalid_share") {
		logger?.(`Invalid share submitted by ${event.identifier} for group ${groupId}`);
		missingSharesFrom.push(event.identifier);
		actions.push({
			id: "key_gen_complain",
			groupId,
			accused: event.identifier,
		});
	}
	// Share collection is completed when every paritcipant submitted a share, no matter if valid or invalid
	// `response` will only be "shares_completed" when all valid shares have been received
	if (!event.shared) {
		logger?.(`Group ${groupId} secret shares not completed yet`);
		return {
			rollover: {
				...machineStates.rollover,
				missingSharesFrom,
				lastParticipant: event.identifier,
			},
			actions,
		};
	}
	// All secret shares collected, now each participant must confirm or complain
	logger?.(`Group ${groupId} secret shares completed, triggering confirmation`);

	if (response === "shares_completed") {
		const nextEpoch = machineStates.rollover.nextEpoch;
		const callbackContext = buildKeyGenCallback(machineConfig, nextEpoch);
		actions.push({
			id: "key_gen_confirm",
			groupId,
			callbackContext,
		});
	}

	return {
		rollover: {
			id: "collecting_confirmations",
			groupId,
			nextEpoch: machineStates.rollover.nextEpoch,
			complaintDeadline: event.block + machineConfig.keyGenTimeout,
			responseDeadline: event.block + 2n * machineConfig.keyGenTimeout,
			deadline: event.block + 3n * machineConfig.keyGenTimeout,
			lastParticipant: event.identifier,
			complaints: machineStates.rollover.complaints,
			missingSharesFrom,
			confirmationsFrom: [],
		},
		actions,
	};
};
