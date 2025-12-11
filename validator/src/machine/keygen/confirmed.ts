import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction, ShieldnetProtocol } from "../../consensus/protocol/types.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { EpochRolloverPacket } from "../../consensus/verify/rollover/schemas.js";
import type { KeyGenConfirmedEvent } from "../transitions/types.js";
import type { ConsensusDiff, ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleKeyGenConfirmed = async (
	machineConfig: MachineConfig,
	protocol: ShieldnetProtocol,
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
	const sharesFrom = [...machineStates.rollover.sharesFrom, event.identifier];
	const participants = signingClient.participants(groupId);
	const allConfirmed = participants.every((p) => sharesFrom.includes(p));

	logger?.(`Group ${groupId} confirmation from ${event.identifier} (${sharesFrom.length}/${participants.length})`);

	// Still waiting for confirmations
	if (!allConfirmed) {
		return {
			rollover: {
				...machineStates.rollover,
				sharesFrom,
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
		return { consensus, rollover: { id: "waiting_for_rollover" }, actions };
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
	const message = await verificationEngine.verify(packet);
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
			responsible: event.identifier,
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
