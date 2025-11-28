import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import { transactionProposedEventSchema } from "../../consensus/schemas.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import type { ConsensusState, MachineConfig, StateDiff } from "../types.js";

export const handleTransactionProposed = async (
	machineConfig: MachineConfig,
	protocol: ShieldnetProtocol,
	verificationEngine: VerificationEngine,
	consensusState: ConsensusState,
	block: bigint,
	eventArgs: unknown,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	// Parse event from raw data
	const event = transactionProposedEventSchema.parse(eventArgs);
	const group = consensusState.epochGroups.get(event.epoch);
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
				epoch: consensusState.activeEpoch,
				signers: machineConfig.defaultParticipants.map((p) => p.id),
				deadline: block + machineConfig.signingTimeout,
			},
		],
	};
};
