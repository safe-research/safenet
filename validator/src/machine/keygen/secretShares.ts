import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction, ShieldnetProtocol } from "../../consensus/protocol/types.js";
import { keyGenSecretSharedEventSchema } from "../../consensus/schemas.js";
import type { SigningClient } from "../../consensus/signing/client.js";
import type { VerificationEngine } from "../../consensus/verify/engine.js";
import type { EpochRolloverPacket } from "../../consensus/verify/rollover/schemas.js";
import type { ConsensusDiff, ConsensusState, MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleKeyGenSecretShared = async (
	machineConfig: MachineConfig,
	protocol: ShieldnetProtocol,
	verificationEngine: VerificationEngine,
	keyGenClient: KeyGenClient,
	signingClient: SigningClient,
	consensusState: ConsensusState,
	machineStates: MachineStates,
	block: bigint,
	eventArgs: unknown,
	logger?: (msg: unknown) => void,
): Promise<StateDiff> => {
	// A participant has submitted secret share for new group
	// Ignore if not in "collecting_shares" state
	if (machineStates.rollover.id !== "collecting_shares") {
		logger?.(`Unexpected state ${machineStates.rollover.id}`);
		return {};
	}
	// Parse event from raw data
	const event = keyGenSecretSharedEventSchema.parse(eventArgs);
	// Verify that the group corresponds to the next epoch
	if (machineStates.rollover.groupId !== event.gid) {
		logger?.(`Unexpected groupId ${event.gid}`);
		return {};
	}
	const groupId = event.gid;
	// Track identity that has submitted last share
	// TODO: handle bad shares -> Submit fraud proof
	await keyGenClient.handleKeygenSecrets(event.gid, event.identifier, event.share.f);
	if (!event.completed) {
		logger?.(`Group ${event.gid} not completed yet`);
		return {
			rollover: {
				...machineStates.rollover,
				lastParticipant: event.identifier,
			},
		};
	}

	// If a group is setup start preprocess (aka nonce commitment)
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

	if (consensusState.genesisGroupId === groupId) {
		logger?.("Genesis group ready!");
		return { consensus, rollover: { id: "waiting_for_rollover" }, actions };
	}
	if (machineStates.rollover.lastParticipant === undefined) {
		throw new Error("Invalid state");
	}
	const nextEpoch = machineStates.rollover.nextEpoch;
	const groupKey = keyGenClient.groupPublicKey(groupId);
	if (groupKey === undefined) {
		throw new Error("Invalid state");
	}
	// The deadline is either the timeout or when the epoch should start
	const packet: EpochRolloverPacket = {
		type: "epoch_rollover_packet",
		domain: {
			chain: protocol.chainId(),
			consensus: protocol.consensus(),
		},
		rollover: {
			activeEpoch: consensusState.activeEpoch,
			proposedEpoch: nextEpoch,
			rolloverBlock: nextEpoch * machineConfig.blocksPerEpoch,
			groupKeyX: groupKey.x,
			groupKeyY: groupKey.y,
		},
	};
	const message = await verificationEngine.verify(packet);
	logger?.(`Verified epoch rollover message ${message}`);
	return {
		consensus,
		rollover: {
			id: "sign_rollover",
			groupId,
			nextEpoch,
			message,
			responsible: machineStates.rollover.lastParticipant,
		},
		signing: [
			message,
			{
				id: "waiting_for_request",
				responsible: machineStates.rollover.lastParticipant,
				packet,
				signers: machineConfig.defaultParticipants.map((p) => p.id),
				deadline: block + machineConfig.signingTimeout,
			},
		],
		actions,
	};
};
