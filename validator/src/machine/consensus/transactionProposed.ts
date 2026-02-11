import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import type { Logger } from "../../utils/logging.js";
import type { TransactionProposedEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, StateDiff } from "../types.js";

export const handleTransactionProposed = async (
	machineConfig: MachineConfig,
	protocol: SafenetProtocol,
	verificationEngine: VerificationEngine,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	event: TransactionProposedEvent,
	logger?: Logger,
): Promise<StateDiff> => {
	const group = consensusState.epochGroups[event.epoch.toString()];
	if (group === undefined) {
		logger?.info?.(`Unknown epoch ${event.epoch}!`);
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
	const result = await verificationEngine.verify(packet);
	if (result.status === "invalid") {
		// Invalid packed, don't update state
		logger?.info?.("Invalid message", { tx: event.transaction, error: result.error });
		return {};
	}
	const message = result.packetId;
	logger?.info?.(`Verified message ${message}`, { tx: event.transaction });
	// The signing will be triggered in a separate event
	const signers = signingClient.participants(group.groupId);
	return {
		signing: [
			message,
			{
				id: "waiting_for_request",
				responsible: undefined,
				packet,
				signers,
				deadline: event.block + machineConfig.signingTimeout,
			},
		],
	};
};
