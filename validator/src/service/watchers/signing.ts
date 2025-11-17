import type { Address, Hex, Log, PublicClient } from "viem";
import { COORDINATOR_EVENTS } from "../../types/abis.js";
import { AbiPoint } from "../../types/interfaces.js";

export const watchSignEvents = ({
	client,
	target,
	onNewNonceCommitmentsHash,
	onSignRequest,
	onNonceCommitmentsRevealed,
	onSignatureShare,
	onError,
}: {
	client: PublicClient;
	target: Address;
	onNewNonceCommitmentsHash: (args: {
		gid?: Hex;
		identifier?: bigint;
		chunk?: number;
		commitment?: Hex;
	}) => Promise<void>;
	onSignRequest: (args: {
		gid?: Hex;
		sid?: Hex;
		message?: Hex;
		sequence?: bigint;
	}) => Promise<void>;
	onNonceCommitmentsRevealed: (args: {
		sid?: Hex;
		identifier?: bigint;
		nonces?: { d: AbiPoint, e: AbiPoint };
	}) => Promise<void>;
	onSignatureShare: (args: {
		sid?: Hex;
		identifier?: bigint;
		z?: bigint;
	}) => Promise<void>;
	onError: (error: Error) => void;
}): (() => void) => {
	// TODO: Allow to specify "fromBlock" to pick up on past events
	// TODO: Provide callback for "last block processed" for service recovery
	return client.watchContractEvent({
		address: target,
		abi: COORDINATOR_EVENTS,
		onLogs: (logs) => {
			logs.forEach((log) => {
				switch (log.eventName) {
					case "Preprocess":
						onNewNonceCommitmentsHash(log.args).catch(onError);
						return;
					case "Sign":
						onSignRequest(log.args).catch(onError);
						return;
					case "SignRevealedNonces":
						onNonceCommitmentsRevealed(log.args).catch(onError);
						return;
					case "SignShare":
						onSignatureShare(log.args)?.catch(onError);
						return;
					default:
						// Event unrelated to signing flow
						return;
				}
			});
		},
		onError,
	});
};