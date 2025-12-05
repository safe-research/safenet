import { encodeFunctionData, type Hex, zeroHash } from "viem";
import type { SigningClient } from "../../consensus/signing/client.js";
import { metaTxHash } from "../../consensus/verify/safeTx/hashing.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import { CONSENSUS_FUNCTIONS } from "../../types/abis.js";
import type { NonceCommitmentsEvent } from "../transitions/types.js";
import type { ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleRevealedNonces = async (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	event: NonceCommitmentsEvent,
): Promise<StateDiff> => {
	// Check that this is a request related to a message that is handled"
	const message = consensusState.signatureIdToMessage[event.sid];
	if (message === undefined) return {};
	// Check that state for signature id is "collect_nonce_commitments"
	const status = machineStates.signing[message];
	if (status?.id !== "collect_nonce_commitments") return {};
	const readyToSubmit = signingClient.handleNonceCommitments(event.sid, event.identifier, {
		hidingNonceCommitment: event.nonces.d,
		bindingNonceCommitment: event.nonces.e,
	});
	if (!readyToSubmit)
		return {
			signing: [
				message,
				{
					...status,
					lastSigner: event.identifier,
				},
			],
		};
	// If all participants have committed update state for request id to "collect_signing_shares"
	const { signersRoot, signersProof, groupCommitment, commitmentShare, signatureShare, lagrangeCoefficient } =
		signingClient.createSignatureShare(event.sid);

	const callbackContext =
		machineStates.rollover.id === "sign_rollover" && machineStates.rollover.message === message
			? encodeFunctionData({
					abi: CONSENSUS_FUNCTIONS,
					functionName: "stageEpoch",
					args: [
						machineStates.rollover.nextEpoch,
						machineStates.rollover.nextEpoch * machineConfig.blocksPerEpoch,
						machineStates.rollover.groupId,
						zeroHash,
					],
				})
			: status.packet.type === "safe_transaction_packet"
				? buildTransactionAttestationCallback(status.packet)
				: undefined;
	return {
		signing: [
			message,
			{
				id: "collect_signing_shares",
				signatureId: status.signatureId,
				sharesFrom: [],
				deadline: event.block + machineConfig.signingTimeout,
				lastSigner: event.identifier,
				packet: status.packet,
			},
		],
		actions: [
			{
				id: "sign_publish_signature_share",
				signatureId: event.sid,
				signersRoot,
				signersProof,
				groupCommitment,
				commitmentShare,
				signatureShare,
				lagrangeCoefficient,
				callbackContext,
			},
		],
	};
};

const buildTransactionAttestationCallback = (packet: SafeTransactionPacket): Hex | undefined => {
	return encodeFunctionData({
		abi: CONSENSUS_FUNCTIONS,
		functionName: "attestTransaction",
		args: [packet.proposal.epoch, metaTxHash(packet.proposal.transaction), zeroHash],
	});
};
