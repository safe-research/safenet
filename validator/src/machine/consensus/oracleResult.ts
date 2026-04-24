import type { SigningClient } from "../../consensus/signing/client.js";
import type { Logger } from "../../utils/logging.js";
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
	if (!event.approved) {
		logger?.info?.("Oracle rejected transaction, dropping state", { requestId: event.requestId });
		return { signing: [event.requestId] };
	}
	logger?.info?.("Oracle approved transaction, participating in signing", { requestId: event.requestId });
	const { nonceCommitments, nonceProof } = signingClient.createNonceCommitments(
		status.gid,
		machineConfig.account,
		status.signatureId,
		event.requestId,
		status.sequence,
		status.signers,
	);
	return {
		consensus: { signatureIdToMessage: [status.signatureId, event.requestId] },
		signing: [
			event.requestId,
			{
				id: "collect_nonce_commitments",
				signatureId: status.signatureId,
				deadline: event.block + machineConfig.signingTimeout,
				lastSigner: undefined,
				packet: status.packet,
			},
		],
		actions: [{ id: "sign_reveal_nonce_commitments", signatureId: status.signatureId, nonceCommitments, nonceProof }],
	};
};
