import { signRequestEventSchema } from "../../consensus/schemas.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import { decodeSequence } from "../../consensus/signing/nonces.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

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
	// TODO: this can be lifted out of this function
	const diff = checkAvailableNonces(signingClient, consensusState, machineStates, event.sequence, logger);
	const status = machineStates.signing[event.message];
	// Check that there is no state or it is the retry flow
	if (status?.id !== "waiting_for_request") {
		logger?.(`Unexpected signing request for ${event.message}!`);
		return diff;
	}
	// Check that message is verified
	if (!verificationEngine.isVerified(event.message)) {
		logger?.(`Message ${event.message} not verified!`);
		return diff;
	}

	const consensus = {
		...diff.consensus,
	};
	consensus.signatureIdToMessage = [event.sid, event.message];

	const { nonceCommitments, nonceProof } = signingClient.createNonceCommitments(
		event.gid,
		event.sid,
		event.message,
		event.sequence,
		status.signers,
	);

	const actions = diff.actions ?? [];
	actions.push({
		id: "sign_reveal_nonce_commitments",
		signatureId: event.sid,
		nonceCommitments,
		nonceProof,
	});
	return {
		consensus,
		signing: [
			event.message,
			{
				id: "collect_nonce_commitments",
				signatureId: event.sid,
				deadline: block + machineConfig.signingTimeout,
				lastSigner: undefined,
				packet: status.packet,
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
): Pick<StateDiff, "consensus"> & Pick<StateDiff, "actions"> => {
	if (consensusState.activeEpoch === 0n && machineStates.rollover.id !== "waiting_for_rollover") {
		// We are in the genesis setup
		return {};
	}
	const activeGroup = consensusState.epochGroups[consensusState.activeEpoch.toString()];
	if (activeGroup !== undefined && !consensusState.groupPendingNonces[activeGroup.groupId] === true) {
		const groupId = activeGroup.groupId;
		let { chunk, offset } = decodeSequence(sequence);
		let availableNonces = 0n;
		while (true) {
			const noncesInChunk = signingClient.availableNoncesCount(activeGroup.groupId, chunk);
			availableNonces += noncesInChunk - offset;
			// Chunk has no nonces, meaning the chunk was not initialized yet.
			if (noncesInChunk === 0n) break;
			// Offset for next chunk should be 0 as it was not used yet
			chunk++;
			offset = 0n;
		}
		if (availableNonces < NONCE_THRESHOLD) {
			logger?.(`Commit nonces for ${groupId}!`);
			const nonceTreeRoot = signingClient.generateNonceTree(groupId);

			return {
				consensus: {
					groupPendingNonces: [groupId, true],
				},
				actions: [
					{
						id: "sign_register_nonce_commitments",
						groupId,
						nonceCommitmentsHash: nonceTreeRoot,
					},
				],
			};
		}
	}
	return {};
};
