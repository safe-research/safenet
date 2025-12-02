import { encodePacked } from "viem";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import { keyGenCommittedEventSchema } from "../../consensus/schemas.js";
import { toPoint } from "../../frost/math.js";
import type {
	ConsensusState,
	MachineConfig,
	MachineStates,
	StateDiff,
} from "../types.js";

export const handleKeyGenCommitted = async (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	eventArgs: unknown,
): Promise<StateDiff> => {
	// A participant has committed to the new key gen
	// Ignore if not in "collecting_commitments" state
	if (machineStates.rollover.id !== "collecting_commitments") return {};
	// Parse event from raw data
	const event = keyGenCommittedEventSchema.parse(eventArgs);
	// Verify that the group corresponds to the next epoch
	if (machineStates.rollover.groupId !== event.gid) return {};
	const nextEpoch = machineStates.rollover.nextEpoch;
	// TODO: handle bad commitments -> Remove participant
	keyGenClient.handleKeygenCommitment(
		event.gid,
		event.identifier,
		event.commitment.c.map((c) => toPoint(c)),
		{
			r: toPoint(event.commitment.r),
			mu: event.commitment.mu,
		},
	);
	if (!event.committed) {
		return {};
	}
	// If all participants have committed update state to "collecting_shares"
	const { verificationShare, shares } = keyGenClient.createSecretShares(
		event.gid,
	);
	const callbackContext =
		consensusState.genesisGroupId === event.gid
			? undefined
			: encodePacked(
					["uint256", "uint256"],
					[nextEpoch, nextEpoch * machineConfig.blocksPerEpoch],
				);
	return {
		rollover: {
			id: "collecting_shares",
			groupId: event.gid,
			nextEpoch,
			deadline: block + machineConfig.keyGenTimeout,
		},
		actions: [
			{
				id: "key_gen_publish_secret_shares",
				groupId: event.gid,
				verificationShare,
				shares,
				callbackContext,
			},
		],
	};
};
