import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { Participant } from "../../consensus/storage/types.js";
import type { MachineConfig, MachineStates, RolloverState, StateDiff } from "../types.js";
import { calcGroupContext } from "./group.js";
import { triggerKeyGen } from "./trigger.js";

const handleCollectingConfirmations = (
	keyGenClient: KeyGenClient,
	rollover: Extract<RolloverState, { id: "collecting_confirmations" }>,
	block: bigint,
): [Participant[], bigint] | undefined => {
	if (rollover.responseDeadline <= block) {
		// Check if there are any responses that timed out
		const unresponded = new Set(
			Object.entries(rollover.complaints)
				.filter(([_, c]) => c.unresponded > 0)
				.map(([id]) => BigInt(id)),
		);
		if (unresponded.size > 0) {
			const currentPariticipants = keyGenClient.participants(rollover.groupId);
			return [currentPariticipants.filter((p) => !unresponded.has(p.id)), rollover.nextEpoch];
		}
	}
	if (rollover.deadline <= block) {
		// Check if confirmations timed out
		const confirmedSet = new Set(rollover.confirmationsFrom);
		const currentPariticipants = keyGenClient.participants(rollover.groupId);
		return [currentPariticipants.filter((p) => confirmedSet.has(p.id)), rollover.nextEpoch];
	}
	// Still within deadline
	return undefined;
};

const handleCollectingCommitments = (
	keyGenClient: KeyGenClient,
	rollover: Extract<RolloverState, { id: "collecting_commitments" }>,
	block: bigint,
): [Participant[], bigint] | undefined => {
	if (rollover.deadline > block) {
		// Still within deadline
		return undefined;
	}
	const missingParticipants = new Set(keyGenClient.missingCommitments(rollover.groupId));
	const currentPariticipants = keyGenClient.participants(rollover.groupId);
	return [currentPariticipants.filter((p) => !missingParticipants.has(p.id)), rollover.nextEpoch];
};

const handleCollectingShares = (
	keyGenClient: KeyGenClient,
	rollover: Extract<RolloverState, { id: "collecting_shares" }>,
	block: bigint,
): [Participant[], bigint] | undefined => {
	if (rollover.deadline > block) {
		// Still within deadline
		return undefined;
	}
	const missingParticipants = new Set(keyGenClient.missingSecretShares(rollover.groupId));
	const currentPariticipants = keyGenClient.participants(rollover.groupId);
	return [currentPariticipants.filter((p) => !missingParticipants.has(p.id)), rollover.nextEpoch];
};

const getTimeoutInfo = (
	keyGenClient: KeyGenClient,
	rollover: RolloverState,
	block: bigint,
): [Participant[], bigint] | undefined => {
	switch (rollover.id) {
		case "collecting_commitments": {
			return handleCollectingCommitments(keyGenClient, rollover, block);
		}
		case "collecting_shares": {
			return handleCollectingShares(keyGenClient, rollover, block);
		}
		case "collecting_confirmations": {
			return handleCollectingConfirmations(keyGenClient, rollover, block);
		}
		default: {
			return undefined;
		}
	}
};

export const checkKeyGenTimeouts = (
	machineConfig: MachineConfig,
	protocol: SafenetProtocol,
	keyGenClient: KeyGenClient,
	machineStates: MachineStates,
	block: bigint,
	logger?: (msg: unknown, span?: unknown) => void,
): StateDiff => {
	const timeoutInfo = getTimeoutInfo(keyGenClient, machineStates.rollover, block);

	if (timeoutInfo === undefined) {
		// No need to adjust participants, as no timeout
		return {};
	}

	logger?.("Key gen timed out", { rollover: { id: machineStates.rollover.id }, timeoutInfo });
	const [adjustedParticipants, nextEpoch] = timeoutInfo;

	// For next key gen only consider active participants
	return triggerKeyGen(
		machineConfig,
		keyGenClient,
		nextEpoch,
		block + machineConfig.keyGenTimeout,
		adjustedParticipants,
		calcGroupContext(protocol.consensus(), nextEpoch),
		logger,
	);
};
