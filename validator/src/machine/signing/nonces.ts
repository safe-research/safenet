import { encodeFunctionData, type Hex, zeroHash } from "viem";
import { nonceCommitmentsEventSchema } from "../../consensus/schemas.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import { toPoint } from "../../frost/math.js";
import { CONSENSUS_FUNCTIONS } from "../../types/abis.js";
import type {
	ConsensusState,
	MachineConfig,
	MachineStates,
	StateDiff,
} from "../types.js";

export const handleRevealedNonces = async (
	machineConfig: MachineConfig,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	eventArgs: unknown,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	// A participant has submitted nonces for a signature id
	// Parse event from raw data
	const event = nonceCommitmentsEventSchema.parse(eventArgs);
	// Check that state for signature id is "collect_nonce_commitments"
	const status = machineStates.signing.get(event.sid);
	if (status?.id !== "collect_nonce_commitments") return {};
	machineStates.signing.set(event.sid, {
		...status,
		lastSigner: event.identifier,
	});
	const message = signingClient.message(event.sid);
	const readyToSubmit = signingClient.handleNonceCommitments(
		event.sid,
		event.identifier,
		{
			hidingNonceCommitment: toPoint(event.nonces.d),
			bindingNonceCommitment: toPoint(event.nonces.e),
		},
	);
	if (!readyToSubmit) return {};
	// If all participants have committed update state for request id to "collect_signing_shares"
	const {
		signersRoot,
		signersProof,
		groupCommitment,
		commitmentShare,
		signatureShare,
		lagrangeCoefficient,
	} = signingClient.createSignatureShare(event.sid);

	const callbackContext =
		machineStates.rollover.id === "sign_rollover" &&
		machineStates.rollover.message === message
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
			: buildTransactionAttestationCallback(consensusState, message, logger);
	return {
		signing: [
			event.sid,
			{
				id: "collect_signing_shares",
				sharesFrom: [],
				deadline: block + machineConfig.signingTimeout,
				lastSigner: event.identifier,
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

const buildTransactionAttestationCallback = (
	consensusState: ConsensusState,
	message: Hex,
	logger?: (msg: unknown) => void,
): Hex | undefined => {
	const info = consensusState.transactionProposalInfo.get(message);
	if (info === undefined) {
		logger?.(`Warn: Unknown proposal info for ${message}`);
		return undefined;
	}
	return encodeFunctionData({
		abi: CONSENSUS_FUNCTIONS,
		functionName: "attestTransaction",
		args: [info.epoch, info.transactionHash, zeroHash],
	});
};
