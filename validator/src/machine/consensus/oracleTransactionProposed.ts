import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { OracleTransactionPacket } from "../../consensus/verify/oracleTx/schemas.js";
import type { Logger } from "../../utils/logging.js";
import type { OracleTransactionProposedEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, StateDiff } from "../types.js";

export const handleOracleTransactionProposed = async (
	machineConfig: MachineConfig,
	protocol: SafenetProtocol,
	verificationEngine: VerificationEngine,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	event: OracleTransactionProposedEvent,
	logger?: Logger,
): Promise<StateDiff> => {
	const groupId = consensusState.epochGroups[event.epoch.toString()];
	if (groupId === undefined) {
		logger?.debug?.(`Unknown epoch ${event.epoch}!`);
		return {};
	}
	if (!signingClient.hasParticipant(groupId, machineConfig.account)) {
		logger?.debug?.(`Not part of signing group ${groupId}!`);
		return {};
	}
	const packet: OracleTransactionPacket = {
		type: "oracle_transaction_packet",
		domain: {
			chain: protocol.chainId(),
			consensus: protocol.consensus(),
		},
		proposal: {
			epoch: event.epoch,
			oracle: event.oracle,
			transaction: event.transaction,
		},
	};
	const result = await verificationEngine.verify(packet);
	const span = { epoch: event.epoch, safeTxHash: event.safeTxHash, oracle: event.oracle };
	if (result.status === "invalid") {
		logger?.info?.(`Invalid oracle transaction packet: ${result.error.message}`, span);
		return {};
	}
	const message = result.packetId;
	logger?.info?.(`Verified oracle transaction packet: ${message}`, span);
	const signers = signingClient.participants(groupId);
	return {
		signing: [
			message,
			{
				id: "wait_for_oracle",
				oracle: event.oracle,
				packet,
				signers,
				deadline: event.block + machineConfig.oracleTimeout,
			},
		],
	};
};
