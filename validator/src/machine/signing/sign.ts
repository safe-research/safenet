import type { ProtocolAction } from "../../consensus/protocol/types.js";
import { signRequestEventSchema } from "../../consensus/schemas.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import { decodeSequence } from "../../consensus/signing/nonces.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type {
	ConsensusState,
	MachineConfig,
	MachineStates,
	StateDiff,
} from "../types.js";

const NONCE_THRESHOLD = 100n;

export const handleSign = async (
	machineConfig: MachineConfig,
	verificationEngine: VerificationEngine,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	eventArgs: unknown,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	// The signature process has been started
	// Parse event from raw data
	const event = signRequestEventSchema.parse(eventArgs);
	const actions = checkAvailableNonces(
		signingClient,
		consensusState,
		machineStates,
		event.sequence,
		logger,
	);
	const status = machineStates.signing.get(event.sid);
	// Check that there is no state or it is the retry flow
	if (status !== undefined && status.id !== "waiting_for_request") {
		logger?.(`Alreay started signing ${event.sid}!`);
		return { actions };
	}
	// Check that signing was initiated via consensus contract
	// TODO: filter by group id
	// Check that message is verified
	if (!verificationEngine.isVerified(event.message)) {
		logger?.(`Message ${event.message} not verified!`);
		return { actions };
	}
	// Check if there is already a request for this message
	const signatureRequest = consensusState.messageSignatureRequests.get(
		event.message,
	);
	// Only allow one concurrent signing process per message
	if (signatureRequest !== undefined) {
		logger?.(`Message ${event.message} is already being signed!`);
		return { actions };
	}
	// TODO: refactor into state diff
	consensusState.messageSignatureRequests.set(event.message, event.sid);

	const signers = status?.signers ?? machineConfig.defaultParticipants.map(p => p.id)
	const { nonceCommitments, nonceProof } = signingClient.createNonceCommitments(
		event.gid,
		event.sid,
		event.message,
		event.sequence,
		signers,
	);

	actions.push({
		id: "sign_reveal_nonce_commitments",
		signatureId: event.sid,
		nonceCommitments,
		nonceProof,
	});
	return {
		signing: [
			event.sid,
			{
				id: "collect_nonce_commitments",
				deadline: block + machineConfig.signingTimeout,
				lastSigner: undefined,
			},
		],
		actions,
	};
};

const checkAvailableNonces = (
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	sequence: bigint,
	logger?: (msg: unknown) => void,
): ProtocolAction[] => {
	if (
		consensusState.activeEpoch === 0n &&
		machineStates.rollover.id !== "waiting_for_rollover"
	) {
		// We are in the genesis setup
		return [];
	}
	const activeGroup = consensusState.epochGroups.get(
		consensusState.activeEpoch,
	);
	if (
		activeGroup !== undefined &&
		!consensusState.groupPendingNonces.has(activeGroup)
	) {
		let { chunk, offset } = decodeSequence(sequence);
		let availableNonces = 0n;
		while (true) {
			const noncesInChunk = signingClient.availableNoncesCount(
				activeGroup,
				chunk,
			);
			availableNonces += noncesInChunk - offset;
			// Chunk has no nonces, meaning the chunk was not initialized yet.
			if (noncesInChunk === 0n) break;
			// Offset for next chunk should be 0 as it was not used yet
			chunk++;
			offset = 0n;
		}
		if (availableNonces < NONCE_THRESHOLD) {
			// TODO: refactor to state diff
			consensusState.groupPendingNonces.add(activeGroup);
			logger?.(`Commit nonces for ${activeGroup}!`);
			const nonceTreeRoot = signingClient.generateNonceTree(activeGroup);

			return [
				{
					id: "sign_register_nonce_commitments",
					groupId: activeGroup,
					nonceCommitmentsHash: nonceTreeRoot,
				},
			];
		}
	}
	return [];
};
