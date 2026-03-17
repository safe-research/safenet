import type { SigningClient } from "../../consensus/signing/client.js";
import type { NonceCommitmentsHashEvent } from "../transitions/types.js";
import type { ConsensusDiff, ConsensusState, MachineConfig, StateDiff } from "../types.js";

export const handlePreprocess = async (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	event: NonceCommitmentsHashEvent,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	// The commited nonces need to be linked to a specific chunk
	// This can happen in any state
	logger?.(`Link nonces for chunk ${event.chunk}`);
	// Clear pending nonce commitments for group
	const consensus: ConsensusDiff = {};
	if (consensusState.groupPendingNonces[event.gid] === true) {
		consensus.groupPendingNonces = [event.gid];
	}
	if (event.participant === machineConfig.account) {
		signingClient.handleNonceCommitmentsHash(event.gid, machineConfig.account, event.commitment, event.chunk);
	}
	return { consensus };
};
