import {
	type Account,
	type Chain,
	createPublicClient,
	createWalletClient,
	extractChain,
	http,
	type PublicClient,
	type Transport,
	webSocket,
} from "viem";
import { KeyGenClient } from "../consensus/keyGen/client.js";
import { OnchainProtocol } from "../consensus/protocol/onchain.js";
import { SigningClient } from "../consensus/signing/client.js";
import { InMemoryStorage } from "../consensus/storage/inmemory.js";
import {
	type PacketHandler,
	type Typed,
	VerificationEngine,
} from "../consensus/verify/engine.js";
import { EpochRolloverHandler } from "../consensus/verify/rollover/handler.js";
import { SafeTransactionHandler } from "../consensus/verify/safeTx/handler.js";
import { CONSENSUS_EVENTS, COORDINATOR_EVENTS } from "../types/abis.js";
import { supportedChains } from "../types/chains.js";
import type { ProtocolConfig } from "../types/interfaces.js";
import type { Logger } from "../utils/logging.js";
import { ShieldnetStateMachine } from "./machine.js";

export class ValidatorService {
	#logger?: Logger;
	#config: ProtocolConfig;
	#publicClient: PublicClient;
	#stateMachine: ShieldnetStateMachine;
	#cleanupCallbacks: (() => void)[] = [];

	constructor({
		account,
		transport,
		config,
		chain,
		logger,
	}: {
		account: Account;
		transport: Transport;
		config: ProtocolConfig;
		chain: Chain;
		logger?: Logger;
	}) {
		this.#logger = logger;
		this.#config = config;
		this.#publicClient = createPublicClient({ chain, transport });
		const walletClient = createWalletClient({ chain, transport, account });
		const storage = new InMemoryStorage(account.address);
		const signingClient = new SigningClient(storage);
		const keyGenClient = new KeyGenClient(storage, this.#logger);
		const verificationHandlers = new Map<string, PacketHandler<Typed>>();
		verificationHandlers.set(
			"safe_transaction_packet",
			new SafeTransactionHandler(),
		);
		verificationHandlers.set(
			"epoch_rollover_packet",
			new EpochRolloverHandler(),
		);
		const verificationEngine = new VerificationEngine(verificationHandlers);
		const protocol = new OnchainProtocol(
			this.#publicClient,
			walletClient,
			config.conensus,
			config.coordinator,
			this.#logger?.info,
		);
		this.#stateMachine = new ShieldnetStateMachine({
			participants: config.participants,
			blocksPerEpoch: config.blocksPerEpoch,
			logger: this.#logger?.info,
			protocol,
			keyGenClient,
			signingClient,
			verificationEngine,
		});
	}

	async start() {
		if (this.#cleanupCallbacks.length > 0) throw Error("Already started!");
		// TODO: from block should be last synced block (after state machine transition)
		this.#cleanupCallbacks.push(
			this.#publicClient.watchContractEvent({
				address: [this.#config.conensus, this.#config.coordinator],
				abi: [...CONSENSUS_EVENTS, ...COORDINATOR_EVENTS],
				fromBlock: 0n,
				onLogs: async (logs) => {
					logs.sort((left, right) => {
						if (left.blockNumber !== right.blockNumber) {
							return left.blockNumber < right.blockNumber ? -1 : 1;
						}
						return left.logIndex - right.logIndex;
					});
					for (const log of logs) {
						this.#stateMachine.transition({
							type: "event",
							block: log.blockNumber,
							index: log.logIndex,
							eventName: log.eventName,
							eventArgs: log.args,
						});
					}
				},
				onError: this.#logger?.error,
			}),
		);
		this.#cleanupCallbacks.push(
			this.#publicClient.watchBlockNumber({
				onBlockNumber: (block) => {
					// We delay the processing to avoid potential race conditions for now
					setTimeout(() => {
						this.#stateMachine.transition({
							type: "block",
							block,
						});
					}, 2000);
				},
				onError: this.#logger?.error,
			}),
		);
	}

	stop() {
		const cleanupCallbacks = this.#cleanupCallbacks;
		this.#cleanupCallbacks = [];
		for (const callback of cleanupCallbacks) {
			callback();
		}
	}
}

export const createValidatorService = (
	account: Account,
	rpcUrl: string,
	config: ProtocolConfig,
): ValidatorService => {
	const transport = rpcUrl.startsWith("wss") ? webSocket(rpcUrl) : http(rpcUrl);
	const chain = extractChain({
		chains: supportedChains,
		id: config.chainId,
	});
	const logger: Logger = {
		error: console.error,
		debug: console.debug,
		info: console.info,
	};
	return new ValidatorService({ account, transport, config, chain, logger });
};
