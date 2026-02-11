import type { Hex } from "viem";
import type { SigningClient } from "../../consensus/signing/client.js";
import { safeTxHash } from "../../consensus/verify/safeTx/hashing.js";
import type { ConsensusState, MachineConfig, MachineStates, SigningState, StateDiff } from "../types.js";

export const checkSigningTimeouts = (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	logger: (msg: unknown, span?: unknown) => void,
): StateDiff[] => {
	const statesToProcess = Object.entries(machineStates.signing) as [Hex, SigningState][];
	const diffs: StateDiff[] = [];
	for (const [signatureId, status] of statesToProcess) {
		diffs.push(
			checkSigningRequestTimeout(
				machineConfig,
				signingClient,
				consensusState,
				machineStates,
				block,
				signatureId,
				status,
				logger,
			),
		);
	}
	return diffs;
};

const checkSigningRequestTimeout = (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	message: Hex,
	status: SigningState,
	logger: (msg: unknown, span?: unknown) => void,
): StateDiff => {
	// Still within deadline
	if (status.deadline > block) return {};
	logger?.(`Signing request ${status.id} timed out`, { signingStatus: status });
	const stateDiff: StateDiff = {};
	switch (status.id) {
		case "waiting_for_attestation": {
			// Remove pending request
			stateDiff.consensus = {
				signatureIdToMessage: [status.signatureId, undefined],
			};
			const everyoneResponsible = status.responsible === undefined;
			if (everyoneResponsible) {
				// Everyone is responsible
				// Signature request will be re-added once it is submitted
				// and no more state needs to be tracked
				// if the deadline is hit again this would be a critical failure
				stateDiff.signing = [message, undefined];
			} else {
				// Make everyone responsible for next retry
				stateDiff.signing = [
					message,
					{
						...status,
						responsible: undefined,
						deadline: block + machineConfig.signingTimeout,
					},
				];
			}
			const signatureId = status.signatureId;
			const act =
				everyoneResponsible ||
				status.responsible === signingClient.participantId(signingClient.signingGroup(signatureId));
			if (!act) {
				return stateDiff;
			}
			if (machineStates.rollover.id === "sign_rollover" && message === machineStates.rollover.message) {
				return {
					...stateDiff,
					actions: [
						{
							id: "consensus_stage_epoch",
							proposedEpoch: machineStates.rollover.nextEpoch,
							rolloverBlock: machineStates.rollover.nextEpoch * machineConfig.blocksPerEpoch,
							groupId: machineStates.rollover.groupId,
							signatureId,
						},
					],
				};
			}
			if (status.packet.type === "safe_transaction_packet") {
				const transactionHash = safeTxHash(status.packet.proposal.transaction);
				return {
					...stateDiff,
					actions: [
						{
							id: "consensus_attest_transaction",
							epoch: status.packet.proposal.epoch,
							transactionHash,
							signatureId,
						},
					],
				};
			}
			return stateDiff;
		}
		case "waiting_for_request": {
			const signers = status.signers.filter((id) => id !== status.responsible);
			const epoch =
				status.packet.type === "epoch_rollover_packet"
					? status.packet.rollover.activeEpoch
					: status.packet.proposal.epoch;
			const groupInfo = consensusState.epochGroups[epoch.toString()];
			if (groupInfo === undefined || signers.length < signingClient.threshold(groupInfo.groupId)) {
				// There are not enough signers, or group is not known, so the request is dropped
				return {
					...stateDiff,
					signing: [message, undefined],
				};
			}
			const { groupId, participantId } = groupInfo;
			const everyoneResponsible = status.responsible === undefined;
			if (everyoneResponsible) {
				// Everyone is responsible
				// Signature request will be re-added once it is submitted
				// and no more state needs to be tracked
				// if the deadline is hit again this would be a critical failure
				stateDiff.signing = [message, undefined];
			} else {
				// Make everyone responsible for next retry
				stateDiff.signing = [
					message,
					{
						...status,
						signers,
						responsible: undefined,
						deadline: block + machineConfig.signingTimeout,
					},
				];
			}
			const act = everyoneResponsible || status.responsible === participantId;
			if (!act) {
				return stateDiff;
			}
			return {
				...stateDiff,
				actions: [
					{
						id: "sign_request",
						groupId,
						message,
					},
				],
			};
		}
		case "collect_nonce_commitments":
		case "collect_signing_shares": {
			// Remove pending request
			stateDiff.consensus = {
				signatureIdToMessage: [status.signatureId, undefined],
			};
			// Get participants that did not participate
			const currentSigners = signingClient.signers(status.signatureId);
			const missingParticipants =
				status.id === "collect_nonce_commitments"
					? signingClient.missingNonces(status.signatureId)
					: currentSigners.filter((s) => status.sharesFrom.indexOf(s) < 0);
			logger?.("Removing signers for not participating", { missingParticipants });
			const epoch =
				status.packet.type === "epoch_rollover_packet"
					? status.packet.rollover.activeEpoch
					: status.packet.proposal.epoch;
			const groupInfo = consensusState.epochGroups[epoch.toString()];
			// There should always be a group for a packet that was accepted before
			if (groupInfo === undefined) {
				throw new Error(`Unknown group for epoch ${epoch}`);
			}
			const { groupId } = groupInfo;
			// For retry of remove inactive signers from current signers set
			const signers = currentSigners.filter((pid) => missingParticipants.indexOf(pid) < 0);
			if (signers.length < signingClient.threshold(groupId)) {
				// Not enough signers to handle the message, remove request
				return {
					...stateDiff,
					signing: [message, undefined],
				};
			}
			stateDiff.signing = [
				message,
				{
					id: "waiting_for_request",
					responsible: status.lastSigner,
					signers,
					deadline: block + machineConfig.signingTimeout,
					packet: status.packet,
				},
			];
			if (status.lastSigner !== signingClient.participantId(groupId)) {
				return stateDiff;
			}
			return {
				...stateDiff,
				actions: [
					{
						id: "sign_request",
						groupId,
						message,
					},
				],
			};
		}
	}
};
