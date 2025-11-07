import type { Address, Hex, Log, PublicClient } from "viem";
import { CONSENSUS_CORE_EVENTS, COORDINATOR_EVENTS } from "../types/abis.js";

type Point = { x: bigint; y: bigint };

export const watchCoordinatorEvents = ({
	client,
	target,
	onKeyGenInit,
	onKeyGenCommitment,
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
	}) => void;
	onKeyGenCommitment: (args: {
		id?: Hex;
		index?: bigint;
		commitment?: { mu: bigint; r: Point; c: readonly Point[] };
	}) => void;
	onError: (error: Error) => void;
	onUnknown?: (log: Log) => void;
}): (() => void) => {
	return client.watchContractEvent({
		address: target,
		abi: COORDINATOR_EVENTS,
		onLogs: (logs) => {
			logs.forEach((log) => {
				switch (log.eventName) {
					case "KeyGen":
						// Handle Approve event
						onKeyGenInit(log.args);
						return;
					case "KeyGenCommitted":
						// Handle Approve event
						onKeyGenCommitment(log.args);
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

export const watchConsusEvents = ({
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
