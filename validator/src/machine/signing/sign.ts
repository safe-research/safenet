import type { SigningClient } from "../../consensus/signing/client.js";
import { decodeSequence } from "../../consensus/signing/nonces.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { Logger } from "../../utils/logging.js";
import type { SignRequestEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";
import { buildNonceCommitmentsDiff } from "./commitments.js";

const NONCE_THRESHOLD = 100n;

export const handleSign = async (
	machineConfig: MachineConfig,
	verificationEngine: VerificationEngine,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	event: SignRequestEvent,
	logger?: Logger,
): Promise<StateDiff> => {
	if (!signingClient.hasParticipant(event.gid, machineConfig.account)) {
		logger?.debug?.(`Not part of signing group ${event.gid}!`);
		return {};
	}
	const diff = checkAvailableNonces(
		machineConfig,
		signingClient,
		consensusState,
		machineStates,
		event.sequence,
		logger,
	);
	const status = machineStates.signing[event.message];
	// Check that there is no state or it is the retry flow
	if (status?.id !== "waiting_for_request") {
		logger?.debug?.(`Unexpected signing request for ${event.message}!`);
		return diff;
	}
	// Check that message is verified, this should not happend in this state
	if (!verificationEngine.isVerified(event.message)) {
		logger?.warn?.(`Message ${event.message} not verified!`);
		return diff;
	}

	// Oracle packet: wait for oracle approval before participating in signing
	if (status.packet.type === "oracle_transaction_packet") {
		return {
			...diff,
			signing: [
				event.message,
				{
					id: "waiting_for_oracle",
					oracle: status.packet.proposal.oracle,
					gid: event.gid,
					signatureId: event.sid,
					sequence: event.sequence,
					signers: status.signers,
					deadline: event.block + machineConfig.oracleTimeout,
					packet: status.packet,
				},
			],
		};
	}

	const ncDiff = buildNonceCommitmentsDiff(machineConfig, signingClient, {
		gid: event.gid,
		signatureId: event.sid,
		message: event.message,
		sequence: event.sequence,
		signers: status.signers,
		block: event.block,
		packet: status.packet,
	});
	return {
		...ncDiff,
		consensus: { ...diff.consensus, ...ncDiff.consensus },
		actions: [...(diff.actions ?? []), ...(ncDiff.actions ?? [])],
	};
};

const checkAvailableNonces = (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	sequence: bigint,
	logger?: Logger,
): Pick<StateDiff, "consensus"> & Pick<StateDiff, "actions"> => {
	if (consensusState.activeEpoch === 0n && machineStates.rollover.id !== "epoch_staged") {
		// We are in the genesis setup
		return {};
	}
	const groupId = consensusState.epochGroups[consensusState.activeEpoch.toString()];
	if (groupId !== undefined && !consensusState.groupPendingNonces[groupId] === true) {
		let { chunk, offset } = decodeSequence(sequence);
		let availableNonces = 0n;
		while (true) {
			const noncesInChunk = signingClient.availableNoncesCount(groupId, machineConfig.account, chunk);
			availableNonces += noncesInChunk - offset;
			// Chunk has no nonces, meaning the chunk was not initialized yet.
			if (noncesInChunk === 0n) break;
			// Offset for next chunk should be 0 as it was not used yet
			chunk++;
			offset = 0n;
		}
		if (availableNonces < NONCE_THRESHOLD) {
			logger?.info?.(`Commit nonces for ${groupId}!`);
			const nonceTreeRoot = signingClient.generateNonceTree(groupId, machineConfig.account);

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
