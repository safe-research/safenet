import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { KeyGenCommittedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleKeyGenCommitted = async (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	machineStates: MachineStates,
	event: KeyGenCommittedEvent,
	logger?: (msg: unknown, span?: unknown) => void,
): Promise<StateDiff> => {
	// A participant has committed to the new key gen
	// Ignore if not in "collecting_commitments" state
	if (machineStates.rollover.id !== "collecting_commitments") return {};
	// Verify that the group corresponds to the next epoch
	if (machineStates.rollover.groupId !== event.gid) return {};
	const nextEpoch = machineStates.rollover.nextEpoch;

	// TODO: [observe mode] allow to collect commitments as non participant
	if (!keyGenClient.participants(event.gid).includes(machineConfig.account)) {
		return {};
	}

	if (
		!keyGenClient.handleKeygenCommitment(event.gid, event.participant, event.commitment.q, event.commitment.c, {
			r: event.commitment.r,
			mu: event.commitment.mu,
		})
	) {
		logger?.(`Invalid key gen commitment from participant ${event.participant}`);
		// No state changes for invalid key gen commitments, participant will be removed on timeout
		return {};
	}
	logger?.(`Registered key gen commitment for participant ${event.participant}`);
	if (!event.committed) {
		return {};
	}
	// TODO: [observe mode] don't generate secrets and perform actions in observe mode BUT build group public key
	// If all participants have committed update state to "collecting_shares"
	const { verificationShare, shares } = keyGenClient.createSecretShares(event.gid, machineConfig.account);
	return {
		rollover: {
			id: "collecting_shares",
			groupId: event.gid,
			nextEpoch,
			deadline: event.block + machineConfig.keyGenTimeout,
			missingSharesFrom: [],
			complaints: {},
		},
		actions: [
			{
				id: "key_gen_publish_secret_shares",
				groupId: event.gid,
				verificationShare,
				shares,
			},
		],
	};
};
