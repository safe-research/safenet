import { encodeFunctionData, type Hex, zeroHash } from "viem";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { OracleTransactionPacket } from "../../consensus/verify/oracleTx/schemas.js";
import { safeTxStructHash } from "../../consensus/verify/safeTx/hashing.js";
import type { SafeTransactionPacket } from "../../consensus/verify/safeTx/schemas.js";
import { CONSENSUS_FUNCTIONS } from "../../types/abis.js";
import type { NonceCommitmentsEvent } from "../transitions/types.js";
import type { BaseSigningState, ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

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
	const readyToSubmit = signingClient.handleNonceCommitments(event.sid, event.participant, {
		hidingNonceCommitment: event.nonces.d,
		bindingNonceCommitment: event.nonces.e,
	});
	if (!readyToSubmit)
		return {
			signing: [
				message,
				{
					...status,
					lastSigner: event.participant,
				},
			],
		};
	// If all participants have committed update state for request id to "collect_signing_shares"
	const { signersRoot, signersProof, groupCommitment, commitmentShare, signatureShare, lagrangeCoefficient } =
		signingClient.createSignatureShare(event.sid, machineConfig.account);

	const callbackContext = buildCallbackContext(machineConfig, machineStates, message, status.packet);
	return {
		signing: [
			message,
			{
				id: "collect_signing_shares",
				signatureId: status.signatureId,
				sharesFrom: [],
				deadline: event.block + machineConfig.signingTimeout,
				lastSigner: event.participant,
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

const buildCallbackContext = (
	machineConfig: MachineConfig,
	machineStates: MachineStates,
	message: Hex,
	packet: BaseSigningState["packet"],
): Hex | undefined => {
	if (machineStates.rollover.id === "sign_rollover" && machineStates.rollover.message === message) {
		return encodeFunctionData({
			abi: CONSENSUS_FUNCTIONS,
			functionName: "stageEpoch",
			args: [
				machineStates.rollover.nextEpoch,
				machineStates.rollover.nextEpoch * machineConfig.blocksPerEpoch,
				machineStates.rollover.groupId,
				zeroHash,
			],
		});
	}
	if (packet.type === "safe_transaction_packet") {
		return buildTransactionAttestationCallback(packet);
	}
	if (packet.type === "oracle_transaction_packet") {
		return buildOracleTransactionAttestationCallback(packet);
	}
	return undefined;
};

const buildTransactionAttestationCallback = (packet: SafeTransactionPacket): Hex | undefined => {
	const { chainId, safe, ...transactionData } = packet.proposal.transaction;
	return encodeFunctionData({
		abi: CONSENSUS_FUNCTIONS,
		functionName: "attestTransaction",
		args: [packet.proposal.epoch, chainId, safe, safeTxStructHash(transactionData), zeroHash],
	});
};

const buildOracleTransactionAttestationCallback = (packet: OracleTransactionPacket): Hex => {
	const { chainId, safe, ...transactionData } = packet.proposal.transaction;
	return encodeFunctionData({
		abi: CONSENSUS_FUNCTIONS,
		functionName: "attestOracleTransaction",
		args: [packet.proposal.epoch, packet.proposal.oracle, chainId, safe, safeTxStructHash(transactionData), zeroHash],
	});
};
