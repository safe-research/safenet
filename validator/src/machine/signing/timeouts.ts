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
	_logger?: (msg: unknown) => void,
): StateDiff[] => {
	// No timeout in waiting state
	const statesToProcess = Array.from(machineStates.signing.entries());
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
	// TODO: refactor into statediff
	consensusState.signatureIdToMessage.delete(message);
	switch (status.id) {
		case "waiting_for_attestation": {
			const everyoneResponsible = status.responsible === undefined;
			const stateDiff: StateDiff = {};
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
			const stateDiff: StateDiff = {};
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
			const groupInfo = consensusState.epochGroups.get(status.epoch);
			if (groupInfo === undefined) {
				throw Error(`Unknown group for epoch ${status.epoch}`);
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
			// Still within deadline
			if (status.deadline <= block) return {};
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
			const groupInfo = consensusState.epochGroups.get(status.epoch);
			if (groupInfo === undefined) {
				throw Error(`Unknown group for epoch ${status.epoch}`);
			}
			const { groupId } = groupInfo;
			return {
				signing: [
					message,
					{
						id: "waiting_for_request",
						responsible: status.lastSigner,
						signers,
						deadline: block + machineConfig.signingTimeout,
						epoch: status.epoch,
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
