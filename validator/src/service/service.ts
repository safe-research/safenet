import Sqlite3, { type Database } from "better-sqlite3";
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
import { SqliteActionQueue } from "../consensus/protocol/sqlite.js";
import type { ActionWithTimeout } from "../consensus/protocol/types.js";
import { SigningClient } from "../consensus/signing/client.js";
import { InMemoryClientStorage } from "../consensus/storage/inmemory.js";
import { SqliteClientStorage } from "../consensus/storage/sqlite.js";
import { type PacketHandler, type Typed, VerificationEngine } from "../consensus/verify/engine.js";
import { EpochRolloverHandler } from "../consensus/verify/rollover/handler.js";
import { SafeTransactionHandler } from "../consensus/verify/safeTx/handler.js";
import { InMemoryStateStorage } from "../machine/storage/inmemory.js";
import { SqliteStateStorage } from "../machine/storage/sqlite.js";
import { OnchainTransitionWatcher } from "../machine/transitions/watcher.js";
import { supportedChains } from "../types/chains.js";
import type { ProtocolConfig } from "../types/interfaces.js";
import type { Logger } from "../utils/logging.js";
import type { Metrics } from "../utils/metrics.js";
import { InMemoryQueue } from "../utils/queue.js";
import { ShieldnetStateMachine } from "./machine.js";

export class ValidatorService {
	#logger: Logger;
	#config: ProtocolConfig;
	#publicClient: PublicClient;
	#watcher: OnchainTransitionWatcher;
	#stateMachine: ShieldnetStateMachine;

	constructor({
		account,
		transport,
		config,
		chain,
		logger,
		metrics,
		database,
	}: {
		account: Account;
		transport: Transport;
		config: ProtocolConfig;
		chain: Chain;
		logger: Logger;
		metrics: Metrics;
		database?: Database;
	}) {
		this.#logger = logger;
		this.#config = config;
		this.#publicClient = createPublicClient({ chain, transport });
		const walletClient = createWalletClient({ chain, transport, account });
		const storage =
			database !== undefined
				? new SqliteClientStorage(account.address, database)
				: new InMemoryClientStorage(account.address);
		const signingClient = new SigningClient(storage);
		const keyGenClient = new KeyGenClient(storage, this.#logger);
		const verificationHandlers = new Map<string, PacketHandler<Typed>>();
		verificationHandlers.set("safe_transaction_packet", new SafeTransactionHandler());
		verificationHandlers.set("epoch_rollover_packet", new EpochRolloverHandler());
		const verificationEngine = new VerificationEngine(verificationHandlers);
		const actionStorage =
			database !== undefined ? new SqliteActionQueue(database) : new InMemoryQueue<ActionWithTimeout>();
		const protocol = new OnchainProtocol(
			this.#publicClient,
			walletClient,
			config.consensus,
			config.coordinator,
			actionStorage,
			this.#logger,
		);
		const stateStorage = database !== undefined ? new SqliteStateStorage(database) : new InMemoryStateStorage();
		this.#stateMachine = new ShieldnetStateMachine({
			participants: config.participants,
			blocksPerEpoch: config.blocksPerEpoch,
			logger: this.#logger,
			metrics,
			genesisSalt: config.genesisSalt,
			protocol,
			storage: stateStorage,
			keyGenClient,
			signingClient,
			verificationEngine,
		});
		this.#watcher = new OnchainTransitionWatcher({
			publicClient: this.#publicClient,
			database: database ?? new Sqlite3(":memory:"),
			config,
			logger,
			onTransition: (t) => {
				this.#stateMachine.transition(t);
			},
		});
	}

	async start() {
		await this.#watcher.start();
	}

	stop() {
		this.#watcher.stop();
	}
}

export const createValidatorService = ({
	account,
	rpcUrl,
	storageFile,
	config,
	logger,
	metrics,
}: {
	account: Account;
	rpcUrl: string;
	storageFile?: string;
	config: ProtocolConfig;
	logger: Logger;
	metrics: Metrics;
}): ValidatorService => {
	const transport = rpcUrl.startsWith("wss") ? webSocket(rpcUrl) : http(rpcUrl);
	const chain = extractChain({
		chains: supportedChains,
		id: config.chainId,
	});
	const database = storageFile !== undefined ? new Sqlite3(storageFile) : undefined;
	return new ValidatorService({ account, transport, config, chain, logger, metrics, database });
};
