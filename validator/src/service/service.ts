import {
	type Chain,
	createPublicClient,
	http,
	type PublicClient,
	type Transport,
	webSocket,
} from "viem";
import { gnosis } from "viem/chains";
import { CONSENSUS_CORE_ABI } from "../types/abis.js";
import type { ConsensusConfig } from "../types/interfaces.js";

export class ValidatorService {
	#config: ConsensusConfig;
	#client: PublicClient;
	#unwatch: (() => void) | null = null;
	constructor(
		transport: Transport,
		config: ConsensusConfig,
		chain: Chain = gnosis,
	) {
		this.#client = createPublicClient({ chain, transport });
		this.#config = config;
	}

	async start() {
		if (this.#unwatch !== null) throw Error("Already started!");
		this.#unwatch = this.#client.watchContractEvent({
			address: this.#config.coreAddress,
			abi: CONSENSUS_CORE_ABI,
			eventName: "Transfer",
			// You can filter for indexed parameters here.
			// For example, to only listen to transfers *to* a specific address:
			// args: {
			//   to: '0xYourAddressHere'
			// },
			onLogs: (logs) => {
				console.log("--- New Transfer Event(s) Received ---");
				logs.forEach((log) => {
					// The `args` are automatically parsed for you!
					const { from, to, value } = log.args;
					console.log(
						`  From: ${from}\n  To: ${to}\n  Value: ${value?.toString()}`,
					);
					console.log(
						`  Block: ${log.blockNumber}\n  Tx Hash: ${log.transactionHash}`,
					);
				});
				console.log("--------------------------------------");
			},
			// This will be called if an error occurs
			onError: (error) => {
				// TODO: handle error
				console.error("An error occurred with the event listener:");
				console.error(error);
			},
		});
	}

	stop() {
		const unwatch = this.#unwatch;
		this.#unwatch = null;
		unwatch?.();
	}
}

export const createValidatorService = (
	rpcUrl: string,
	config: ConsensusConfig,
): ValidatorService => {
	const transport = rpcUrl.startsWith("wss") ? webSocket(rpcUrl) : http(rpcUrl);
	return new ValidatorService(transport, config);
};
