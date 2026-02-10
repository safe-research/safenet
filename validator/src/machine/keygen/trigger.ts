import type { Hex } from "viem";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { Participant } from "../../consensus/storage/types.js";
import type { MachineConfig, StateDiff } from "../types.js";
import { calcMinimumParticipants, calcTreshold } from "./group.js";

export const triggerKeyGen = (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	epoch: bigint,
	deadline: bigint,
	participants: Participant[],
	context: Hex,
	logger?: (msg: unknown) => void,
): StateDiff => {
	const requiredParticipants = calcMinimumParticipants(machineConfig);
	if (participants.length < requiredParticipants) {
		// TODO: error refactor - skip -> introduce epoch_skipped
		throw new Error(`Not enough participants! Expected at least ${requiredParticipants} got ${participants.length}`);
	}
	const count = participants.length;
	const threshold = calcTreshold(count);
	const { groupId, participantsRoot, participantId, commitments, pok, poap } = keyGenClient.setupGroup(
		participants,
		threshold,
		context,
	);

	const actions: ProtocolAction[] = [
		{
			id: "key_gen_start",
			participants: participantsRoot,
			count,
			threshold,
			context,
			participantId,
			commitments,
			pok,
			poap,
		},
	];

	logger?.(`Triggered key gen for epoch ${epoch} with ${groupId}`);
	return {
		consensus: {
			epochGroup: [epoch, { groupId, participantId }],
		},
		rollover: {
			id: "collecting_commitments",
			nextEpoch: epoch,
			groupId,
			deadline: deadline,
		},
		actions,
	};
};
