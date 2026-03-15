import Sqlite3, { type Database } from "better-sqlite3";
import {
	type Account,
	type Chain,
	type ChainFees,
	createPublicClient,
	createWalletClient,
	extractChain,
	http,
	isAddressEqual,
	type PublicClient,
	type Transport,
	webSocket,
} from "viem";
import { KeyGenClient } from "../consensus/keyGen/client.js";
import { GasFeeEstimator, OnchainProtocol } from "../consensus/protocol/onchain.js";
import { SqliteActionQueue, SqliteTxStorage } from "../consensus/protocol/sqlite.js";
import { SigningClient } from "../consensus/signing/client.js";
import { SqliteClientStorage } from "../consensus/storage/sqlite.js";
import { type PacketHandler, type Typed, VerificationEngine } from "../consensus/verify/engine.js";
import { EpochRolloverHandler } from "../consensus/verify/rollover/handler.js";
import { SafeTransactionHandler } from "../consensus/verify/safeTx/handler.js";
import { SqliteStateStorage } from "../machine/storage/sqlite.js";
import { OnchainTransitionWatcher, type WatcherConfig } from "../machine/transitions/watcher.js";
import type { RolloverState } from "../machine/types.js";
import { CONSENSUS_FUNCTIONS } from "../types/abis.js";
import { supportedChains } from "../types/chains.js";
import type { ProtocolConfig } from "../types/interfaces.js";
import { formatError } from "../utils/errors.js";
import type { Logger } from "../utils/logging.js";
import type { Metrics } from "../utils/metrics/index.js";
import { withMetrics } from "../utils/metrics/transport.js";
import { buildSafeTransactionCheck } from "./checks.js";
import { SafenetStateMachine } from "./machine.js";

export class ValidatorService {
	#logger: Logger;
	#publicClient: PublicClient;
	#watcher: OnchainTransitionWatcher;
	#stateMachine: SafenetStateMachine;
	#setStakerAddress: () => Promise<void>;

	constructor({
		account,
		transport,
		config,
		watcherConfig,
		chain,
		logger,
		metrics,
		database,
		skipGenesis = false,
	}: {
		account: Account;
		transport: Transport;
		config: ProtocolConfig;
		watcherConfig: WatcherConfig;
		chain: Chain;
		logger: Logger;
		metrics: Metrics;
		database: Database;
		skipGenesis?: boolean;
	}) {
		this.#logger = logger;
		this.#publicClient = createPublicClient({ chain, transport });
		const walletClient = createWalletClient({ chain, transport, account });
		const storage = new SqliteClientStorage(account.address, database);
		const signingClient = new SigningClient(storage);
		const keyGenClient = new KeyGenClient(storage, this.#logger);
		const verificationHandlers = new Map<string, PacketHandler<Typed>>();
		const check = buildSafeTransactionCheck();
		verificationHandlers.set("safe_transaction_packet", new SafeTransactionHandler(check, metrics));
		verificationHandlers.set("epoch_rollover_packet", new EpochRolloverHandler());
		const verificationEngine = new VerificationEngine(verificationHandlers);
		const actionStorage = new SqliteActionQueue(database);
		const txStorage = new SqliteTxStorage(database);
		const gasFeeEstimator = new GasFeeEstimator(this.#publicClient);
		const protocol = new OnchainProtocol({
			publicClient: this.#publicClient,
			signingClient: walletClient,
			gasFeeEstimator,
			consensus: config.consensus,
			coordinator: config.coordinator,
			queue: actionStorage,
			txStorage,
			logger: this.#logger,
			blocksBeforeResubmit: config.blocksBeforeResubmit,
		});
		if (skipGenesis) {
			logger.notice("Skipping genesis key gen!");
		}
		const initialRolloverState: RolloverState = skipGenesis ? { id: "skip_genesis" } : { id: "waiting_for_genesis" };
		const stateStorage = new SqliteStateStorage(database, initialRolloverState);
		this.#stateMachine = new SafenetStateMachine({
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
			keyGenTimeout: config.keyGenTimeout,
			signingTimeout: config.signingTimeout,
		});
		this.#watcher = new OnchainTransitionWatcher({
			publicClient: this.#publicClient,
			database,
			config,
			watcherConfig,
			logger,
			onTransition: (t) => {
				this.#stateMachine.transition(t);
				// If new block:
				// - invalidate cached gas fees
				// - check pending actions
				if (t.id === "block_new") {
					gasFeeEstimator.invalidate();
					protocol.checkPendingActions(t.block);
				}
			},
		});
		this.#setStakerAddress = async () => {
			try {
				const currentStaker = await this.#publicClient.readContract({
					address: config.consensus,
					abi: CONSENSUS_FUNCTIONS,
					functionName: "getValidatorStaker",
					args: [account.address],
				});
				if (isAddressEqual(currentStaker, config.staker)) {
					this.#logger.info("Validator staker address is already set correctly.");
					return;
				}
			} catch (error) {
				this.#logger.error("Error while getting the current validator staker address", { error: formatError(error) });
				return;
			}
			this.#logger.info(`Setting validator staker address to ${config.staker}...`);
			protocol.process({ id: "consensus_set_validator_staker", staker: config.staker });
		};
	}

	async start() {
		await this.#setStakerAddress();
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
	skipGenesis,
}: {
	account: Account;
	rpcUrl: string;
	storageFile?: string;
	config: ProtocolConfig;
	watcherConfig: WatcherConfig;
	logger: Logger;
	metrics: Metrics;
	fees?: ChainFees;
	skipGenesis?: boolean;
}): ValidatorService => {
	const transport = withMetrics(rpcUrl.startsWith("wss") ? webSocket(rpcUrl) : http(rpcUrl), metrics);
	const chain: Chain = {
		...extractChain({
			chains: supportedChains,
			id: config.chainId,
		}),
		fees,
	};
	const database = new Sqlite3(storageFile ?? ":memory:");
	return new ValidatorService({
		account,
		transport,
		config,
		watcherConfig,
		chain,
		logger,
		metrics,
		database,
		skipGenesis,
	});
};
