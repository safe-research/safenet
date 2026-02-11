import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction, SafenetProtocol } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { EpochRolloverPacket } from "../../consensus/verify/rollover/schemas.js";
import { jsonReplacer } from "../../utils/json.js";
import type { KeyGenConfirmedEvent } from "../transitions/types.js";
import type { ConsensusDiff, ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleKeyGenConfirmed = async (
	machineConfig: MachineConfig,
	protocol: SafenetProtocol,
	verificationEngine: VerificationEngine,
	keyGenClient: KeyGenClient,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	event: KeyGenConfirmedEvent,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	const block = event.block;
	// A participant has confirmed their participation in the key gen ceremony
	// Ignore if not in "collecting_confirmations" state
	if (machineStates.rollover.id !== "collecting_confirmations") {
		logger?.(`Unexpected state ${machineStates.rollover.id}`);
		return {};
	}
	// Verify that the group corresponds to the expected group
	if (machineStates.rollover.groupId !== event.gid) {
		logger?.(`Unexpected groupId ${event.gid}`);
		return {};
	}
	const groupId = event.gid;

	// Track this confirmation
	const confirmationsFrom = [...machineStates.rollover.confirmationsFrom, event.identifier];
	const participants = signingClient.participants(groupId);
	const allConfirmed = participants.every((p) => confirmationsFrom.includes(p));

	logger?.(
		`Group ${groupId} confirmation from ${event.identifier} (${confirmationsFrom.length}/${participants.length})`,
	);

	// Still waiting for confirmations
	if (!allConfirmed) {
		return {
			rollover: {
				...machineStates.rollover,
				confirmationsFrom,
				lastParticipant: event.identifier,
			},
		};
	}

	// Genesis group: after all confirmations, we're done with keygen
	if (consensusState.genesisGroupId === groupId) {
		// All confirmed for genesis group - start preprocessing and return to waiting state
		logger?.("Genesis group all confirmations received, starting preprocessing");
		const consensus: ConsensusDiff = {
			groupPendingNonces: [groupId, true],
		};
		const nonceTreeRoot = signingClient.generateNonceTree(groupId);
		const actions: ProtocolAction[] = [
			{
				id: "sign_register_nonce_commitments",
				groupId,
				nonceCommitmentsHash: nonceTreeRoot,
			},
		];
		// Epoch 0 (Genesis) is setup. This should trigger the first non-genesis key gen
		return { consensus, rollover: { id: "epoch_staged", nextEpoch: 0n }, actions };
	}

	// All participants have confirmed - compute the epoch rollover message locally
	logger?.(`Group ${groupId} all participants confirmed, computing epoch rollover message`);

	const groupPublicKey = keyGenClient.groupPublicKey(groupId);
	if (!groupPublicKey) {
		throw new Error(`Group public key not available for ${groupId}`);
	}

	const nextEpoch = machineStates.rollover.nextEpoch;
	const rolloverBlock = nextEpoch * machineConfig.blocksPerEpoch;

	// Create the epoch rollover packet
	const packet: EpochRolloverPacket = {
		type: "epoch_rollover_packet",
		domain: {
			chain: protocol.chainId(),
			consensus: protocol.consensus(),
		},
		rollover: {
			activeEpoch: consensusState.activeEpoch,
			proposedEpoch: nextEpoch,
			rolloverBlock,
			groupKeyX: groupPublicKey.x,
			groupKeyY: groupPublicKey.y,
		},
	};

	// Verify the packet to get the message hash
	const result = await verificationEngine.verify(packet);
	if (result.status === "invalid") {
		throw new Error(`Invalid epoch packet created ${JSON.stringify(packet, jsonReplacer)}`);
	}
	const message = result.packetId;
	logger?.(`Computed epoch rollover message ${message}`);

	// Transition to sign_rollover with the proper message
	// Note: Preprocessing (nonce generation) will be triggered after the epoch is staged,
	// as per the spec's epoch_staged handler.
	return {
		rollover: {
			id: "sign_rollover",
			groupId,
			nextEpoch,
			message,
		},
		signing: [
			message,
			{
				id: "waiting_for_request",
				responsible: event.identifier,
				packet,
				signers: participants,
				deadline: block + machineConfig.signingTimeout,
			},
		],
	};
};
