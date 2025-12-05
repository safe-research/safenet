import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import type { TransactionProposedEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, StateDiff } from "../types.js";

export const handleTransactionProposed = async (
	machineConfig: MachineConfig,
	protocol: ShieldnetProtocol,
	verificationEngine: VerificationEngine,
	consensusState: ConsensusState,
	event: TransactionProposedEvent,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	const group = consensusState.epochGroups[event.epoch.toString()];
	if (group === undefined) {
		logger?.(`Unknown epoch ${event.epoch}!`);
		return {};
	}
	const packet: SafeTransactionPacket = {
		type: "safe_transaction_packet",
		domain: {
			chain: protocol.chainId(),
			consensus: protocol.consensus(),
		},
		proposal: {
			epoch: event.epoch,
			transaction: event.transaction,
		},
	};
	const message = await verificationEngine.verify(packet);
	logger?.(`Verified message ${message}`);
	// The signing will be triggered in a separate event
	return {
		signing: [
			message,
			{
				id: "waiting_for_request",
				responsible: undefined,
				packet,
				signers: machineConfig.defaultParticipants.map((p) => p.id),
				deadline: event.block + machineConfig.signingTimeout,
			},
		],
	};
};
