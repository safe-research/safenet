import Sqlite3, { type Database } from "better-sqlite3";
import {
	type Account,
	type Chain,
	type ChainFees,
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
import { SqliteActionQueue, SqliteTxStorage } from "../consensus/protocol/sqlite.js";
import type { ActionWithTimeout } from "../consensus/protocol/types.js";
import { SigningClient } from "../consensus/signing/client.js";
import { InMemoryClientStorage } from "../consensus/storage/inmemory.js";
import { SqliteClientStorage } from "../consensus/storage/sqlite.js";
import { type PacketHandler, type Typed, VerificationEngine } from "../consensus/verify/engine.js";
import { EpochRolloverHandler } from "../consensus/verify/rollover/handler.js";
import { SafeTransactionHandler } from "../consensus/verify/safeTx/handler.js";
import { InMemoryStateStorage } from "../machine/storage/inmemory.js";
import { SqliteStateStorage } from "../machine/storage/sqlite.js";
import { OnchainTransitionWatcher, type WatcherConfig } from "../machine/transitions/watcher.js";
import { supportedChains } from "../types/chains.js";
import type { ProtocolConfig } from "../types/interfaces.js";
import type { Logger } from "../utils/logging.js";
import type { Metrics } from "../utils/metrics.js";
import { InMemoryQueue } from "../utils/queue.js";
import { buildSafeTransactionCheck } from "./checks.js";
import { ShieldnetStateMachine } from "./machine.js";

export class ValidatorService {
	#logger: Logger;
	#publicClient: PublicClient;
	#watcher: OnchainTransitionWatcher;
	#stateMachine: ShieldnetStateMachine;

	constructor({
		account,
		transport,
		config,
		watcherConfig,
		chain,
		logger,
		metrics,
		database,
	}: {
		account: Account;
		transport: Transport;
		config: ProtocolConfig;
		watcherConfig: WatcherConfig;
		chain: Chain;
		logger: Logger;
		metrics: Metrics;
		database?: Database;
	}) {
		this.#logger = logger;
		this.#publicClient = createPublicClient({ chain, transport });
		const walletClient = createWalletClient({ chain, transport, account });
		const storage =
			database !== undefined
				? new SqliteClientStorage(account.address, database)
				: new InMemoryClientStorage(account.address);
		const signingClient = new SigningClient(storage);
		const keyGenClient = new KeyGenClient(storage, this.#logger);
		const verificationHandlers = new Map<string, PacketHandler<Typed>>();
		const check = buildSafeTransactionCheck();
		verificationHandlers.set("safe_transaction_packet", new SafeTransactionHandler(check));
		verificationHandlers.set("epoch_rollover_packet", new EpochRolloverHandler());
		const verificationEngine = new VerificationEngine(verificationHandlers);
		const actionStorage =
			database !== undefined ? new SqliteActionQueue(database) : new InMemoryQueue<ActionWithTimeout>();
		const txStorage = new SqliteTxStorage(database ?? new Sqlite3(":memory:"));
		const protocol = new OnchainProtocol({
			publicClient: this.#publicClient,
			signingClient: walletClient,
			consensus: config.consensus,
			coordinator: config.coordinator,
			queue: actionStorage,
			txStorage,
			logger: this.#logger,
		});
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
			watcherConfig,
			logger,
			onTransition: (t) => {
				this.#stateMachine.transition(t);
				// If we get a new block check pending actions
				if (t.id === "block_new") {
					protocol.checkPendingActions(t.block);
				}
			},
		});
	}

	async start() {
		await this.#watcher.start();
	}

	async stop() {
		await this.#watcher.stop();
	}
}

export const createValidatorService = ({
	account,
	rpcUrl,
	storageFile,
	config,
	watcherConfig,
	logger,
	metrics,
	fees,
}: {
	account: Account;
	rpcUrl: string;
	storageFile?: string;
	config: ProtocolConfig;
	watcherConfig: WatcherConfig;
	logger: Logger;
	metrics: Metrics;
	fees?: ChainFees;
}): ValidatorService => {
	const transport = rpcUrl.startsWith("wss") ? webSocket(rpcUrl) : http(rpcUrl);
	const chain: Chain = {
		...extractChain({
			chains: supportedChains,
			id: config.chainId,
		}),
		fees,
	};
	const database = storageFile !== undefined ? new Sqlite3(storageFile) : undefined;
	return new ValidatorService({ account, transport, config, watcherConfig, chain, logger, metrics, database });
};
