import type { ShieldnetProtocol } from "../../consensus/protocol/types.js";
import { transactionProposedEventSchema } from "../../consensus/schemas.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import type { ConsensusState, StateDiff } from "../types.js";

export const handleTransactionProposed = async (
	protocol: ShieldnetProtocol,
	verificationEngine: VerificationEngine,
	consensusState: ConsensusState,
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
	// TODO: refactor to state diff
	consensusState.transactionProposalInfo.set(message, {
		epoch: event.epoch,
		transactionHash: event.transactionHash,
	});
	logger?.(`Verified message ${message}`);
	// The signing will be triggered in a separate event
	return {};
};
