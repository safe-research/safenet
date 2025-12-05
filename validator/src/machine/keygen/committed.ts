import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { KeyGenCommittedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleKeyGenCommitted = async (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	machineStates: MachineStates,
	event: KeyGenCommittedEvent,
): Promise<StateDiff> => {
	// A participant has committed to the new key gen
	// Ignore if not in "collecting_commitments" state
	if (machineStates.rollover.id !== "collecting_commitments") return {};
	// Verify that the group corresponds to the next epoch
	if (machineStates.rollover.groupId !== event.gid) return {};
	const nextEpoch = machineStates.rollover.nextEpoch;
	// TODO: handle bad commitments -> Remove participant
	keyGenClient.handleKeygenCommitment(event.gid, event.identifier, event.commitment.c, {
		r: event.commitment.r,
		mu: event.commitment.mu,
	});
	if (!event.committed) {
		return {};
	}
	// If all participants have committed update state to "collecting_shares"
	const { verificationShare, shares } = keyGenClient.createSecretShares(event.gid);
	return {
		rollover: {
			id: "collecting_shares",
			groupId: event.gid,
			nextEpoch,
			deadline: event.block + machineConfig.keyGenTimeout,
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
