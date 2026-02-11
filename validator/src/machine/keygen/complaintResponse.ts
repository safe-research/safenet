import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction, SafenetProtocol } from "../../consensus/protocol/types.js";
import type { KeyGenComplaintResponsedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates, RolloverState, StateDiff } from "../types.js";
import { calcGroupContext } from "./group.js";
import { triggerKeyGen } from "./trigger.js";
import { buildKeyGenCallback } from "./utils.js";

export const handleComplaintResponded = async (
	machineConfig: MachineConfig,
	protocol: SafenetProtocol,
	keyGenClient: KeyGenClient,
	machineStates: MachineStates,
	event: KeyGenComplaintResponsedEvent,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	if (machineStates.rollover.id !== "collecting_shares" && machineStates.rollover.id !== "collecting_confirmations") {
		return {};
	}

	if (machineStates.rollover.groupId !== event.gid) {
		return {};
	}

	if (
		machineStates.rollover.id === "collecting_confirmations" &&
		event.block > machineStates.rollover.responseDeadline
	) {
		return {};
	}

	const accusedId = event.accused.toString();
	const complaint = machineStates.rollover.complaints[accusedId];

	if (complaint === undefined || complaint.unresponded === 0n) {
		return {};
	}
	// If reponse is required to finalize shares get state by registering secret, otherwise only verify
	const sharesState =
		keyGenClient.participantId(event.gid) === event.plaintiff &&
		machineStates.rollover.missingSharesFrom.includes(event.accused)
			? await keyGenClient.registerPlainKeyGenSecret(event.gid, event.accused, event.secretShare)
			: !keyGenClient.verifySecretShare(event.gid, event.accused, event.plaintiff, event.secretShare)
				? "invalid_share"
				: undefined;

	if (sharesState === "invalid_share") {
		const participants = keyGenClient
			.participants(machineStates.rollover.groupId)
			.filter((p) => p.id !== event.accused);
		return triggerKeyGen(
			machineConfig,
			keyGenClient,
			machineStates.rollover.nextEpoch,
			event.block + machineConfig.keyGenTimeout,
			participants,
			calcGroupContext(protocol.consensus(), machineStates.rollover.nextEpoch),
			logger,
		);
	}

	const actions: ProtocolAction[] = [];
	if (sharesState === "shares_completed") {
		const nextEpoch = machineStates.rollover.nextEpoch;
		const callbackContext = buildKeyGenCallback(machineConfig, nextEpoch);
		actions.push({
			id: "key_gen_confirm",
			groupId: event.gid,
			callbackContext,
		});
	}

	const missingSharesFrom =
		sharesState === "pending_shares" || sharesState === "shares_completed"
			? machineStates.rollover.missingSharesFrom.filter((p) => p !== event.accused)
			: machineStates.rollover.missingSharesFrom;

	const complaints = {
		...machineStates.rollover.complaints,
		[accusedId]: {
			total: complaint.total,
			unresponded: complaint.unresponded - 1n,
		},
	};

	const rollover: RolloverState = {
		...machineStates.rollover,
		missingSharesFrom,
		complaints,
	};
	return {
		rollover,
		actions,
	};
};
