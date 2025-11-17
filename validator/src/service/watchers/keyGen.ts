import type { Address, Hex, Log, PublicClient } from "viem";
import { COORDINATOR_EVENTS } from "../../types/abis.js";
import { AbiPoint } from "../../types/interfaces.js";

export const watchKeyGenEvents = ({
	client,
	target,
	onKeyGenInit,
	onKeyGenCommitment,
	onKeyGenSecrets,
	onError,
}: {
	client: PublicClient;
	target: Address;
	onKeyGenInit: (args: {
		gid?: Hex;
		participants?: Hex;
		count?: bigint;
		threshold?: bigint;
	}) => Promise<void>;
	onKeyGenCommitment: (args: {
		gid?: Hex;
		identifier?: bigint;
		commitment?: { mu: bigint; r: AbiPoint; c: readonly AbiPoint[] };
	}) => Promise<void>;
	onKeyGenSecrets: (args: {
		gid?: Hex;
		identifier?: bigint;
		share?: { f: readonly bigint[]; y: AbiPoint };
	}) => Promise<void>;
	onError: (error: Error) => void;
}): (() => void) => {
	// TODO: Allow to specify "fromBlock" to pick up on past events
	// TODO: Provide callback for "last block processed" for service recovery
    // TODO: Add keygen aborted
	return client.watchContractEvent({
		address: target,
		abi: COORDINATOR_EVENTS,
		onLogs: (logs) => {
			logs.forEach((log) => {
				switch (log.eventName) {
					case "KeyGen":
						onKeyGenInit(log.args).catch(onError);
						return;
					case "KeyGenCommitted":
						onKeyGenCommitment(log.args).catch(onError);
						return;
					case "KeyGenSecretShared":
						onKeyGenSecrets(log.args).catch(onError);
						return;
					default:
                        // TODO: Add keygen aborted
						// Event unrelated to keygen flow
						return;
				}
			});
		},
		onError,
	});
};