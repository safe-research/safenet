import type { Hex } from "viem";
import type { SigningClient } from "../../consensus/signing/client.js";
import { metaTxHash } from "../../consensus/verify/safeTx/hashing.js";
import type {
	ConsensusState,
	MachineConfig,
	MachineStates,
	SigningState,
	StateDiff,
} from "../types.js";

export const checkSigningTimeouts = (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
): StateDiff[] => {
	const statesToProcess = Object.entries(machineStates.signing) as [
		Hex,
		SigningState,
	][];
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
): StateDiff => {
	// Still within deadline
	if (status.deadline > block) return {};
	const stateDiff: StateDiff = {
		consensus: {
			signatureIdToMessage: [message],
		},
	};
	switch (status.id) {
		case "waiting_for_attestation": {
			const everyoneResponsible = status.responsible === undefined;
			if (everyoneResponsible) {
				// Everyone is responsible
				// Signature request will be readded once it is submitted
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
				status.responsible === signingClient.participantId(signatureId);
			if (!act) {
				return stateDiff;
			}
			if (
				machineStates.rollover.id === "sign_rollover" &&
				message === machineStates.rollover.message
			) {
				return {
					...stateDiff,
					actions: [
						{
							id: "consensus_stage_epoch",
							proposedEpoch: machineStates.rollover.nextEpoch,
							rolloverBlock:
								machineStates.rollover.nextEpoch * machineConfig.blocksPerEpoch,
							groupId: machineStates.rollover.groupId,
							signatureId,
						},
					],
				};
			}
			if (status.packet.type === "safe_transaction_packet") {
				const transactionHash = metaTxHash(status.packet.proposal.transaction);
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
			const everyoneResponsible = status.responsible === undefined;
			if (everyoneResponsible) {
				// Everyone is responsible
				// Signature request will be readded once it is submitted
				// and no more state needs to be tracked
				// if the deadline is hit again this would be a critical failure
				stateDiff.signing = [message, undefined];
			} else {
				// Make everyone responsible for next retry
				stateDiff.signing = [
					message,
					{
						...status,
						signers: status.signers.filter((id) => id !== status.responsible),
						responsible: undefined,
						deadline: block + machineConfig.signingTimeout,
					},
				];
			}
			const epoch =
				status.packet.type === "epoch_rollover_packet"
					? status.packet.rollover.activeEpoch
					: status.packet.proposal.epoch;
			const groupInfo = consensusState.epochGroups[epoch.toString()];
			if (groupInfo === undefined) {
				throw new Error(`Unknown group for epoch ${epoch}`);
			}
			const { groupId, participantId } = groupInfo;
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
			// Get participants that did not participate
			const missingParticipants =
				status.id === "collect_nonce_commitments"
					? signingClient.missingNonces(status.signatureId)
					: signingClient
							.signers(status.signatureId)
							.filter((s) => status.sharesFrom.indexOf(s) < 0);
			// For next key gen only consider active participants
			const signers = machineConfig.defaultParticipants
				.filter((p) => missingParticipants.indexOf(p.id) < 0)
				.map((p) => p.id);
			const epoch =
				status.packet.type === "epoch_rollover_packet"
					? status.packet.rollover.activeEpoch
					: status.packet.proposal.epoch;
			const groupInfo = consensusState.epochGroups[epoch.toString()];
			if (groupInfo === undefined) {
				throw new Error(`Unknown group for epoch ${epoch}`);
			}
			const { groupId } = groupInfo;
			return {
				...stateDiff,
				signing: [
					message,
					{
						id: "waiting_for_request",
						responsible: status.lastSigner,
						signers,
						deadline: block + machineConfig.signingTimeout,
						packet: status.packet,
					},
				],
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
