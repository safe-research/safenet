import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { EpochRolloverPacket } from "../../consensus/verify/rollover/schemas.js";
import type { EpochProposedEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleEpochProposed = async (
	machineConfig: MachineConfig,
	protocol: ShieldnetProtocol,
	verificationEngine: VerificationEngine,
	machineStates: MachineStates,
	event: EpochProposedEvent,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	logger?.(`EpochProposed: epoch ${event.proposedEpoch} with rollover block ${event.rolloverBlock}`);

	// We should be in sign_rollover state after our confirmation
	if (machineStates.rollover.id !== "sign_rollover") {
		logger?.(`Unexpected state ${machineStates.rollover.id} for EpochProposed`);
		return {};
	}

	// Create and verify the epoch rollover packet
	const packet: EpochRolloverPacket = {
		type: "epoch_rollover_packet",
		domain: {
			chain: protocol.chainId(),
			consensus: protocol.consensus(),
		},
		rollover: {
			activeEpoch: event.activeEpoch,
			proposedEpoch: event.proposedEpoch,
			rolloverBlock: event.rolloverBlock,
			groupKeyX: event.groupKey.x,
			groupKeyY: event.groupKey.y,
		},
	};

	// Verify the packet to get the message hash
	const message = await verificationEngine.verify(packet);
	logger?.(`Verified epoch rollover message ${message}`);

	// Set up the signing state for the incoming Sign event
	return {
		rollover: {
			id: "sign_rollover",
			groupId: machineStates.rollover.groupId,
			nextEpoch: event.proposedEpoch,
			message,
			responsible: machineStates.rollover.responsible,
		},
		signing: [
			message,
			{
				id: "waiting_for_request",
				responsible: machineStates.rollover.responsible,
				packet,
				signers: machineConfig.defaultParticipants.map((p) => p.id),
				deadline: event.block + machineConfig.signingTimeout,
			},
		],
	};
};
