import type { SigningClient } from "../../consensus/signing/client.js";
import type { Logger } from "../../utils/logging.js";
import { buildNonceCommitmentsDiff } from "../signing/commitments.js";
import type { OracleResultEvent } from "../transitions/types.js";
import type { MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleOracleResult = async (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	machineStates: MachineStates,
	event: OracleResultEvent,
	logger?: Logger,
): Promise<StateDiff> => {
	const status = machineStates.signing[event.requestId];
	if (status?.id !== "waiting_for_oracle") {
		logger?.debug?.(`No waiting_for_oracle state for oracle result ${event.requestId}`);
		return {};
	}
	if (event.oracle !== status.oracle) {
		logger?.debug?.(`Oracle mismatch for result ${event.requestId}: expected ${status.oracle}, got ${event.oracle}`);
		return {};
	}
	if (!event.approved) {
		logger?.info?.("Oracle rejected transaction, dropping state", { requestId: event.requestId });
		return { signing: [event.requestId] };
	}
	logger?.info?.("Oracle approved transaction, participating in signing", { requestId: event.requestId });
	return buildNonceCommitmentsDiff(machineConfig, signingClient, {
		gid: status.gid,
		signatureId: status.signatureId,
		message: event.requestId,
		sequence: status.sequence,
		signers: status.signers,
		block: event.block,
		packet: status.packet,
	});
};
