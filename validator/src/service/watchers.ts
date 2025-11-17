import type { Address, Hex, Log, PublicClient } from "viem";
import { CONSENSUS_CORE_EVENTS, COORDINATOR_EVENTS } from "../types/abis.js";

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
