import {
	type Chain,
	createPublicClient,
	http,
	type PublicClient,
	type Transport,
	webSocket,
} from "viem";
import { gnosis } from "viem/chains";
import type { ConsensusConfig } from "../types/interfaces.js";
import { watchConsusEvents } from "./watchers.js";

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
		this.#unwatch = watchConsusEvents({
			client: this.#client,
			target: this.#config.coreAddress,
			onApprove: console.log,
			onTransfer: console.log,
			onError: console.error,
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
