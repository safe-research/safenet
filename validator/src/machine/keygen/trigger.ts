import type { Hex } from "viem";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { Participant } from "../../consensus/storage/types.js";
import type { Logger } from "../../utils/logging.js";
import type { MachineConfig, StateDiff } from "../types.js";
import { calcMinimumParticipants, calcTreshold } from "./group.js";

export const triggerKeyGen = (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	epoch: bigint,
	deadline: bigint,
	participants: Participant[],
	context: Hex,
	logger?: Logger,
): StateDiff => {
	const requiredParticipants = calcMinimumParticipants(machineConfig);
	if (participants.length < requiredParticipants) {
		logger?.info?.(`Skipped epoch ${epoch}!`, { requiredParticipants, participants });
		return {
			rollover: {
				id: "epoch_skipped",
				nextEpoch: epoch,
			},
		};
	}
	const count = participants.length;
	const threshold = calcTreshold(count);
	const { groupId, participantsRoot, participantId, commitments, encryptionKey, pok, poap } = keyGenClient.setupGroup(
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
			encryptionKey,
			pok,
			poap,
		},
	];

	logger?.info?.(`Triggered key gen for epoch ${epoch} with ${groupId}`, { participants });
	return {
		consensus: {
			epochGroup: [epoch, { groupId, participantId }],
		},
		rollover: {
			id: "collecting_commitments",
			nextEpoch: epoch,
			groupId,
			deadline,
		},
		actions,
	};
};
