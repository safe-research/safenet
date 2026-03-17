import type { Address, Hex } from "viem";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction } from "../../consensus/protocol/types.js";
import type { Logger } from "../../utils/logging.js";
import type { MachineConfig, StateDiff } from "../types.js";
import { calcMinimumParticipants, calcThreshold } from "./group.js";

export const triggerKeyGen = (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	epoch: bigint,
	deadline: bigint,
	participants: Address[],
	context: Hex,
	logger?: Logger,
): StateDiff => {
	const requiredParticipants = calcMinimumParticipants(machineConfig, epoch);
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
	const threshold = calcThreshold(count);
	const { groupId, participantsRoot } = keyGenClient.setupGroup(participants, threshold, context);
	const actions: ProtocolAction[] = [];
	if (participants.includes(machineConfig.account)) {
		const { commitments, encryptionPublicKey, pok, poap } = keyGenClient.setupKeyGen(
			groupId,
			machineConfig.account,
			participants,
			threshold,
		);
		actions.push({
			id: "key_gen_start",
			participants: participantsRoot,
			count,
			threshold,
			context,
			encryptionPublicKey,
			commitments,
			pok,
			poap,
		});
	}

	logger?.info?.(`Triggered key gen for epoch ${epoch} with ${groupId}`, { participants });
	return {
		consensus: {
			epochGroup: [epoch, groupId],
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
