import { encodeAbiParameters, type Hex } from "viem";
import type { KeyGenClient } from "../../consensus/keyGen/client.js";
import type { ProtocolAction } from "../../consensus/protocol/types.js";
import { keyGenSecretSharedEventSchema } from "../../consensus/schemas.js";
import type { MachineConfig, MachineStates, StateDiff } from "../types.js";

export const handleKeyGenSecretShared = async (
	machineConfig: MachineConfig,
	keyGenClient: KeyGenClient,
	machineStates: MachineStates,
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
		logger?.(`Group ${event.gid} secret shares not completed yet`);
		return {
			rollover: {
				...machineStates.rollover,
				lastParticipant: event.identifier,
			},
		};
	}

	// All secret shares collected, now each participant must confirm
	logger?.(`Group ${event.gid} secret shares completed, triggering confirmation`);

	// Build the callback context for non-genesis group (to trigger epoch proposal after confirmation)
	let callbackContext: Hex | undefined;
	if (machineStates.rollover.nextEpoch !== 0n) {
		// For non-genesis groups, we include callback context to trigger epoch proposal
		const nextEpoch = machineStates.rollover.nextEpoch;
		const rolloverBlock = nextEpoch * machineConfig.blocksPerEpoch;
		// ABI encode: (uint64 proposedEpoch, uint64 rolloverBlock)
		callbackContext = encodeAbiParameters([{ type: "uint64" }, { type: "uint64" }], [nextEpoch, rolloverBlock]);
	}

	const actions: ProtocolAction[] = [
		{
			id: "key_gen_confirm",
			groupId,
			callbackContext,
		},
	];

	return {
		rollover: {
			id: "collecting_confirmations",
			groupId,
			nextEpoch: machineStates.rollover.nextEpoch,
			deadline: machineStates.rollover.deadline,
			lastParticipant: event.identifier,
		},
		actions,
	};
};
