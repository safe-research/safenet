import { nonceCommitmentsHashEventSchema } from "../../consensus/schemas.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { ConsensusState, StateDiff } from "../types.js";

export const handlePreprocess = async (
	signingClient: SigningClient,
	consensusState: ConsensusState,
	eventArgs: unknown,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	// The commited nonces need to be linked to a specific chunk
	// This can happen in any state
	// This will be handled by the signingClient
	const event = nonceCommitmentsHashEventSchema.parse(eventArgs);
	logger?.(`Link nonces for chunk ${event.chunk}`);
	// Clear pending nonce commitments for group
	if (consensusState.groupPendingNonces.has(event.gid)) {
		// TODO: refactor into state diff
		consensusState.groupPendingNonces.delete(event.gid);
	}
	signingClient.handleNonceCommitmentsHash(
		event.gid,
		event.identifier,
		event.commitment,
		event.chunk,
	);
	return {};
};
