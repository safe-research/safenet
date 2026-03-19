import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import { safeTxProposalHash } from "../../consensus/verify/safeTx/hashing.js";
import type { Logger } from "../../utils/logging.js";
import type { TransactionAttestedEvent } from "../transitions/types.js";
import type { MachineStates, StateDiff } from "../types.js";

export const handleTransactionAttested = async (
	protocol: SafenetProtocol,
	machineStates: MachineStates,
	event: TransactionAttestedEvent,
	logger?: Logger,
): Promise<StateDiff> => {
	// Check that signing state is waiting for attestation
	const message = safeTxProposalHash({
		domain: {
			chain: protocol.chainId(),
			consensus: protocol.consensus(),
		},
		proposal: {
			epoch: event.epoch,
			safeTxHash: event.safeTxHash,
		},
	});
	const status = machineStates.signing[message];
	if (status?.id !== "waiting_for_attestation") return {};
	logger?.notice?.(`Attested transaction with hash ${event.safeTxHash}`);

	// Clean up internal state
	return {
		consensus: {
			signatureIdToMessage: [status.signatureId],
		},
		signing: [message],
	};
};
