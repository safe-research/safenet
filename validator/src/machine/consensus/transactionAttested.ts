import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import { safeTxProposalHash } from "../../consensus/verify/safeTx/hashing.js";
import type { TransactionAttestedEvent } from "../transitions/types.js";
import type { MachineStates, StateDiff } from "../types.js";

export const handleTransactionAttested = async (
	protocol: SafenetProtocol,
	machineStates: MachineStates,
	event: TransactionAttestedEvent,
): Promise<StateDiff> => {
	// Check that signing state is waiting for attestation
	const message = safeTxProposalHash({
		domain: {
			chain: protocol.chainId(),
			consensus: protocol.consensus(),
		},
		proposal: {
			epoch: event.epoch,
			safeTxHash: event.transactionHash,
		},
	});
	const status = machineStates.signing[message];
	if (status?.id !== "waiting_for_attestation") return {};

	// Clean up internal state
	return {
		consensus: {
			signatureIdToMessage: [status.signatureId],
		},
		signing: [message],
	};
};
