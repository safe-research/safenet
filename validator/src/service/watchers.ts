import type { Address, Hex, Log, PublicClient } from "viem";
import { CONSENSUS_CORE_EVENTS, COORDINATOR_EVENTS } from "../types/abis.js";

type Point = { x: bigint; y: bigint };

export const watchCoordinatorEvents = ({
	client,
	target,
	onKeyGenInit,
	onKeyGenCommitment,
	onKeyGenSecrets,
	onError,
	onUnknown,
}: {
	client: PublicClient;
	target: Address;
	onKeyGenInit: (args: {
		id?: Hex;
		participants?: Hex;
		count?: bigint;
		threshold?: bigint;
	}) => Promise<void>;
	onKeyGenCommitment: (args: {
		id?: Hex;
		index?: bigint;
		commitment?: { mu: bigint; r: Point; c: readonly Point[] };
	}) => Promise<void>;
	onKeyGenSecrets: (args: {
		id?: Hex;
		index?: bigint;
		share?: { f: readonly bigint[]; y: Point };
	}) => Promise<void>;
	onError: (error: Error) => void;
	onUnknown?: (log: Log) => Promise<void>;
}): (() => void) => {
	// TODO: Allow to specify "fromBlock" to pick up on past events
	// TODO: Provide callback for "last block processed" for service recovery
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
						// TODO: should never happen, check if it can be removed
						// Unknown event
						onUnknown?.(log)?.catch(onError);
						return;
				}
			});
		},
		onError,
	});
};

export const watchConsensusEvents = ({
	client,
	target,
	onApprove,
	onTransfer,
	onError,
	onUnknown,
}: {
	client: PublicClient;
	target: Address;
	onApprove: (args: { from?: Address; to?: Address; amount?: bigint }) => void;
	onTransfer: (args: { from?: Address; to?: Address; value?: bigint }) => void;
	onError: (error: Error) => void;
	onUnknown?: (log: Log) => void;
}): (() => void) => {
	return client.watchContractEvent({
		address: target,
		abi: CONSENSUS_CORE_EVENTS,
		onLogs: (logs) => {
			logs.forEach((log) => {
				switch (log.eventName) {
					case "Approve":
						// Handle Approve event
						onApprove(log.args);
						return;
					case "Transfer":
						// Handle Transfer event
						onTransfer(log.args);
						return;
					default:
						// TODO: should never happen, check if it can be removed
						// Unknown event
						onUnknown?.(log);
						return;
				}
			});
		},
		onError,
	});
};
