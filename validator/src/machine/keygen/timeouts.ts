import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import type { Participant } from "../../consensus/storage/types.js";
import type { MachineConfig, MachineStates, RolloverState, StateDiff } from "../types.js";
import { calcGroupContext } from "./group.js";
import { triggerKeyGen } from "./trigger.js";

type CollectingState = Extract<
	RolloverState,
	{ id: "collecting_commitments" | "collecting_shares" | "collecting_confirmations" }
>;

const adjustParticipants = (
	defaultParticipants: Participant[],
	keyGenClient: KeyGenClient,
	rollover: CollectingState,
): Participant[] => {
	switch (rollover.id) {
		case "collecting_commitments": {
			const missingParticipants = keyGenClient.missingCommitments(rollover.groupId);
			return defaultParticipants.filter((p) => missingParticipants.indexOf(p.id) < 0);
		}
		case "collecting_shares": {
			const missingParticipants = keyGenClient.missingSecretShares(rollover.groupId);
			return defaultParticipants.filter((p) => missingParticipants.indexOf(p.id) < 0);
		}
		case "collecting_confirmations": {
			return defaultParticipants.filter((p) => rollover.sharesFrom.indexOf(p.id) >= 0);
		}
	}
};

export const checkKeyGenTimeouts = (
	machineConfig: MachineConfig,
	protocol: ShieldnetProtocol,
	keyGenClient: KeyGenClient,
	machineStates: MachineStates,
	block: bigint,
	logger?: (msg: unknown) => void,
): StateDiff => {
	// No timeout in waiting state
	// Timeouts in signing state will be handled in the signing flow
	if (
		machineStates.rollover.id !== "collecting_commitments" &&
		machineStates.rollover.id !== "collecting_shares" &&
		machineStates.rollover.id !== "collecting_confirmations"
	) {
		return {};
	}
	// Still within deadline
	if (machineStates.rollover.deadline > block) {
		return {};
	}

	// For next key gen only consider active participants
	const participants = adjustParticipants(machineConfig.defaultParticipants, keyGenClient, machineStates.rollover);
	const { diff } = triggerKeyGen(
		keyGenClient,
		machineStates.rollover.nextEpoch,
		block + machineConfig.keyGenTimeout,
		participants,
		calcGroupContext(protocol.consensus(), machineStates.rollover.nextEpoch),
		logger,
	);
	return diff;
};
