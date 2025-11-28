import type { SigningClient } from "../../consensus/signing/client.js";
import type { SignatureId } from "../../frost/types.js";
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
	signatureId: SignatureId,
	status: SigningState,
): StateDiff => {
	// Still within deadline
	if (status.deadline > block) return {};
	// TODO: refactor into statediff
	consensusState.messageSignatureRequests.delete(signatureId);
	switch (status.id) {
		case "waiting_for_attestation": {
			const everyoneResponsible = status.responsible === undefined;
			const stateDiff: StateDiff = {};
			if (everyoneResponsible) {
				// Everyone is responsible
				// Signature request will be readded once it is submitted
				// and no more state needs to be tracked
				// if the deadline is hit again this would be a critical failure
				stateDiff.signing = [signatureId, undefined];
			} else {
				// Make everyone responsible for next retry
				stateDiff.signing = [
					signatureId,
					{
						...status,
						responsible: undefined,
						deadline: block + machineConfig.signingTimeout,
					},
				];
			}
			const act =
				everyoneResponsible ||
				status.responsible === signingClient.participantId(signatureId);
			if (!act) {
				return stateDiff;
			}
			const message = signingClient.message(signatureId);
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
			const transactionInfo =
				consensusState.transactionProposalInfo.get(message);
			if (transactionInfo !== undefined) {
				return {
					...stateDiff,
					actions: [
						{
							id: "consensus_attest_transaction",
							...transactionInfo,
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
				stateDiff.signing = [signatureId, undefined];
			} else {
				// Make everyone responsible for next retry
				stateDiff.signing = [
					signatureId,
					{
						...status,
						signers: status.signers.filter((id) => id !== status.responsible),
						responsible: undefined,
						deadline: block + machineConfig.signingTimeout,
					},
				];
			}
			const act =
				everyoneResponsible ||
				status.responsible === signingClient.participantId(signatureId);
			if (!act) {
				return stateDiff;
			}
			const message = signingClient.message(signatureId);
			const groupId = signingClient.signingGroup(signatureId);
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
					? signingClient.missingNonces(signatureId)
					: signingClient
							.signers(signatureId)
							.filter((s) => status.sharesFrom.indexOf(s) < 0);
			// For next key gen only consider active participants
			const signers = machineConfig.defaultParticipants
				.filter((p) => missingParticipants.indexOf(p.id) < 0)
				.map((p) => p.id);
			const groupId = signingClient.signingGroup(signatureId);
			const message = signingClient.message(signatureId);
			return {
				signing: [
					signatureId,
					{
						id: "waiting_for_request",
						responsible: status.lastSigner,
						signers,
						deadline: block + machineConfig.signingTimeout,
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
