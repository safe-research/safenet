import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { Logger } from "../../utils/logging.js";
import type { KeyGenSecretSharedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates, StateDiff } from "../types.js";
import { buildKeyGenCallback } from "./utils.js";

export const handleKeyGenSecretShared = async (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	machineStates: MachineStates,
	event: KeyGenSecretSharedEvent,
	logger?: Logger,
): Promise<StateDiff> => {
	// A participant has submitted secret share for new group
	// Ignore if not in "collecting_shares" state
	if (machineStates.rollover.id !== "collecting_shares") {
		logger?.debug?.(`Unexpected state ${machineStates.rollover.id}`);
		return {};
	}
	const groupId = event.gid;

	// Verify that the group corresponds to the next epoch
	if (machineStates.rollover.groupId !== groupId) {
		logger?.debug?.(`Unexpected groupId ${groupId}`);
		return {};
	}

	// Track identity that has submitted last share
	const response = keyGenClient.hasParticipant(groupId, machineConfig.account)
		? await keyGenClient.handleKeygenSecrets(groupId, machineConfig.account, event.participant, event.share.f)
		: undefined;

	const actions: ProtocolAction[] = [];
	const sharesFrom = [...machineStates.rollover.sharesFrom, event.participant];
	if (response === "invalid_share") {
		logger?.notice?.(`Invalid share submitted by ${event.participant} for group ${groupId}`);
		actions.push({
			id: "key_gen_complain",
			groupId,
			accused: event.participant,
		});
	}
	// Share collection is completed when every paritcipant submitted a share, no matter if valid or invalid
	// `response` will only be "shares_completed" when all valid shares have been received
	if (!event.shared) {
		logger?.info?.(`Group ${groupId} secret shares not completed yet`);
		return {
			rollover: {
				...machineStates.rollover,
				sharesFrom,
				lastParticipant: event.participant,
			},
			actions,
		};
	}
	// All secret shares collected, now each participant must confirm or complain
	logger?.info?.(`Group ${groupId} secret shares completed, triggering confirmation`);

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
			lastParticipant: event.participant,
			complaints: machineStates.rollover.complaints,
			sharesFrom,
			confirmationsFrom: [],
		},
		actions,
	};
};
