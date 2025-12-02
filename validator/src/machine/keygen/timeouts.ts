import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import type { MachineConfig, MachineStates, StateDiff } from "../types.js";
import { triggerKeyGen } from "./trigger.js";

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
	if (machineStates.rollover.id !== "collecting_commitments" && machineStates.rollover.id !== "collecting_shares") {
		return {};
	}
	// Still within deadline
	if (machineStates.rollover.deadline > block) {
		return {};
	}
	const groupId = machineStates.rollover.groupId;
	// Get participants that did not participate
	const missingParticipants =
		machineStates.rollover.id === "collecting_commitments"
			? keyGenClient.missingCommitments(groupId)
			: keyGenClient.missingSecretShares(groupId);
	// For next key gen only consider active participants
	const participants = machineConfig.defaultParticipants.filter((p) => missingParticipants.indexOf(p.id) < 0);
	const { diff } = triggerKeyGen(
		keyGenClient,
		machineStates.rollover.nextEpoch,
		block + machineConfig.keyGenTimeout,
		participants,
		protocol.consensus(),
		logger,
	);
	return diff;
};
