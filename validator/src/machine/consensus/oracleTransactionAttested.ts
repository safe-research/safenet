import type { SafenetProtocol } from "../../consensus/protocol/types.js";
import { oracleTxProposalHash } from "../../consensus/verify/oracleTx/hashing.js";
import type { Logger } from "../../utils/logging.js";
import type { OracleTransactionAttestedEvent } from "../transitions/types.js";
import type { MachineStates, StateDiff } from "../types.js";

export const handleOracleTransactionAttested = async (
	protocol: SafenetProtocol,
	machineStates: MachineStates,
	event: OracleTransactionAttestedEvent,
	logger?: Logger,
): Promise<StateDiff> => {
	const message = oracleTxProposalHash({
		domain: {
			chain: protocol.chainId(),
			consensus: protocol.consensus(),
		},
		proposal: {
			epoch: event.epoch,
			oracle: event.oracle,
			safeTxHash: event.safeTxHash,
		},
	});
	const status = machineStates.signing[message];
	if (status?.id !== "waiting_for_attestation") return {};
	logger?.notice?.(`Attested oracle transaction with hash ${event.safeTxHash}`);

	return {
		consensus: {
			signatureIdToMessage: [status.signatureId],
		},
		signing: [message],
	};
};
