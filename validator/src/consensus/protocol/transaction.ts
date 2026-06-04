import {
	type Address,
	type Chain,
	type FeeValuesEIP1559,
	type Hex,
	keccak256,
	NonceTooLowError,
	type PublicClient,
	TransactionExecutionError,
	type Transport,
} from "viem";
import type { ValidatorAccount } from "../../types/account.js";
import { formatError } from "../../utils/errors.js";
import type { Logger } from "../../utils/logging.js";
import { maxBigInt, minBigInt } from "../../utils/math.js";
import type { SubmittedAction } from "./base.js";

export type FeeValues = Pick<FeeValuesEIP1559, "maxFeePerGas" | "maxPriorityFeePerGas">;
export type EthTransactionData = { to: Address; value: bigint; data: Hex; gas?: bigint };
export type EthTransactionDetails = { nonce: number; fees: FeeValues | null; hash: Hex | null };

export interface TransactionStorage {
	register(tx: EthTransactionData, minNonce: number): number;
	countPending(): number;
	delete(nonce: number): void;
	setPending(nonce: number): void;
	setFees(nonce: number, fees: FeeValues): void;
	setHash(nonce: number, txHash: Hex): void;
	setExecutedUpTo(nonce: number): number;
	setSubmittedForPending(blockNumber: bigint): number;
	maxNonce(): number | null;
	submittedUpTo(blockNumber: bigint, offset?: number, limit?: number): (EthTransactionData & EthTransactionDetails)[];
}

export class GasFeeEstimator {
	#cachedPrices: Promise<FeeValues> | null = null;
	#client: PublicClient;
	#priorityFeeCapPercentage: number | undefined;
	#logger: Logger | undefined;

	constructor(client: PublicClient, priorityFeeCapPercentage?: number, logger?: Logger) {
		this.#client = client;
		this.#priorityFeeCapPercentage = priorityFeeCapPercentage;
		this.#logger = logger;
	}

	invalidate() {
		this.#cachedPrices = null;
	}

	estimateFees(): Promise<FeeValues> {
		if (this.#cachedPrices !== null) {
			return this.#cachedPrices;
		}
		// Also cache errors, to prevent that on error too many request are fired
		const pricePromise = this.#client.estimateFeesPerGas().then((fees) => this.#capPriorityFee(fees));
		this.#cachedPrices = pricePromise;
		return pricePromise;
	}

	#capPriorityFee(fees: FeeValues): FeeValues {
		if (this.#priorityFeeCapPercentage === undefined) {
			return fees;
		}

		// Solve for newP such that newP / newF = capPercent / 100.
		// Note that we need to do math in the bigint space, so we scale our percentage amount to allow
		// for up to 6 digits of precision in the `#priorityFeeCapPercentage` parameter.
		const PRECISION = 1_000_000n;
		const scaledPercent = BigInt(Math.round((this.#priorityFeeCapPercentage / 100) * Number(PRECISION)));
		if (scaledPercent >= PRECISION) {
			return fees;
		}

		const baseFeeComponent = fees.maxFeePerGas - fees.maxPriorityFeePerGas;
		const cappedPriority = (baseFeeComponent * scaledPercent) / (PRECISION - scaledPercent);
		const maxPriorityFeePerGas = minBigInt(fees.maxPriorityFeePerGas, cappedPriority);
		if (maxPriorityFeePerGas < fees.maxPriorityFeePerGas) {
			this.#logger?.debug("Priority fee cap applied.", {
				originalMaxPriorityFeePerGas: fees.maxPriorityFeePerGas,
				cappedMaxPriorityFeePerGas: maxPriorityFeePerGas,
			});
		}
		return {
			maxPriorityFeePerGas,
			maxFeePerGas: baseFeeComponent + maxPriorityFeePerGas,
		};
	}
}

export class TransactionManager {
	#publicClient: PublicClient<Transport, Chain>;
	#account: ValidatorAccount;
	#gasFeeEstimator: GasFeeEstimator;
	#txStorage: TransactionStorage;
	#logger: Logger;
	#blocksBeforeResubmit: bigint;
	#queuedPendingCheckBlockNumber: bigint | null = null;
	#runningPendingCheck = false;

	constructor({
		publicClient,
		account,
		gasFeeEstimator,
		txStorage,
		logger,
		blocksBeforeResubmit,
	}: {
		publicClient: PublicClient<Transport, Chain>;
		account: ValidatorAccount;
		gasFeeEstimator: GasFeeEstimator;
		txStorage: TransactionStorage;
		logger: Logger;
		blocksBeforeResubmit?: bigint;
	}) {
		this.#publicClient = publicClient;
		this.#account = account;
		this.#gasFeeEstimator = gasFeeEstimator;
		this.#txStorage = txStorage;
		this.#logger = logger;
		this.#blocksBeforeResubmit = blocksBeforeResubmit ?? 1n;
	}

	isRunningPendingCheck(): boolean {
		return this.#runningPendingCheck;
	}

	triggerPendingCheck(blockNumber: bigint) {
		if (this.#runningPendingCheck) {
			this.#logger.debug(`Queueing pending actions check for block ${blockNumber}`);
			this.#queuedPendingCheckBlockNumber = blockNumber;
			return;
		}
		this.#runningPendingCheck = true;
		this.#checkPendingActionsLoop(blockNumber).finally(() => {
			this.#runningPendingCheck = false;
		});
	}

	async #checkPendingActionsLoop(initialBlockNumber: bigint) {
		let blockForCheck: bigint | null = initialBlockNumber;
		while (blockForCheck !== null) {
			try {
				await this.#checkPendingActions(blockForCheck);
			} catch (e) {
				this.#logger.error("Error while checking pending transactions.", { error: formatError(e) });
			}
			blockForCheck = this.#queuedPendingCheckBlockNumber;
			this.#queuedPendingCheckBlockNumber = null;
		}
	}

	async #checkPendingActions(blockNumber: bigint) {
		// Optimistically check whether or not we have pending actions. If we don't then we can just exit early and
		// save on some RPC calls and database reads.
		if (this.#txStorage.countPending() === 0) {
			return;
		}

		// For transaction without a submission block set it to this block
		// This assumes that the transaction should be included in this block
		// If the blocksBeforeResubmit is 1 block, these transactions will only be retried on the next block
		const newPendingTxs = this.#txStorage.setSubmittedForPending(blockNumber);
		if (newPendingTxs > 0) {
			this.#logger.debug(`Marked ${newPendingTxs} transactions as submitted at block ${blockNumber}`);
		}
		const currentNonce = await this.#publicClient.getTransactionCount({
			address: this.#account.address,
			blockTag: "latest",
		});
		const executedNonce = currentNonce - 1;
		const executedTxs = this.#txStorage.setExecutedUpTo(executedNonce);
		if (executedTxs > 0) {
			this.#logger.debug(
				`Marked ${executedTxs} transactions as executed up to nonce ${executedNonce} on block ${blockNumber}`,
			);
		}
		// Only fetch the first page of pending transactions (default limit is 100) to avoid retrying too many
		// transactions at once.
		const pendingTxs = this.#txStorage.submittedUpTo(blockNumber - this.#blocksBeforeResubmit);
		for (const tx of pendingTxs) {
			this.#logger.debug(`Resubmit transaction for ${tx.nonce}!`, { transaction: tx });
			try {
				await this.submitTransaction(tx);
			} catch (error) {
				if (
					error instanceof NonceTooLowError ||
					// Nonce error might be nested as cause error
					(error instanceof TransactionExecutionError && error.cause instanceof NonceTooLowError)
				) {
					this.#logger.info(`Nonce already used. Marking transaction with nonce ${tx.nonce} as executed!`, {
						transaction: tx,
					});
					this.#txStorage.setExecutedUpTo(tx.nonce);
					continue;
				}
				this.#logger.warn(`Error submitting transaction for ${tx.nonce}!`, { error: formatError(error) });
				// If an error occurs skip rest of pending transactions, to avoid triggering more errors
				return;
			}
		}
	}

	async submitTransaction(tx: EthTransactionData & Pick<EthTransactionDetails, "nonce" | "fees">): Promise<Hex> {
		const estimatedFees = await this.#gasFeeEstimator.estimateFees();
		// Use max of (previous fees + 10%) and estimate
		const fees: FeeValues = {
			maxFeePerGas: maxBigInt(estimatedFees.maxFeePerGas, ((tx.fees?.maxFeePerGas ?? 0n) * 110n) / 100n),
			maxPriorityFeePerGas: maxBigInt(
				estimatedFees.maxPriorityFeePerGas,
				((tx.fees?.maxPriorityFeePerGas ?? 0n) * 110n) / 100n,
			),
		};

		this.#txStorage.setPending(tx.nonce);
		// Store fees before submission in case an error occurs
		this.#txStorage.setFees(tx.nonce, fees);
		const unsignedTx = {
			to: tx.to,
			value: tx.value,
			data: tx.data,
			nonce: tx.nonce,
			gas: tx.gas,
			chainId: this.#publicClient.chain.id,
			...fees,
		};
		const signedTx = await this.#account.signTransaction(unsignedTx);
		const txHash = keccak256(signedTx);
		this.#txStorage.setHash(tx.nonce, txHash);
		this.#logger.debug(`Submitting transaction for nonce ${tx.nonce}!`, { tx: { hash: txHash, ...unsignedTx } });
		return this.#publicClient.sendRawTransaction({ serializedTransaction: signedTx });
	}

	async submitAction(txData: EthTransactionData): Promise<SubmittedAction> {
		// 1. Get Network Baseline (The "Minimum Nonce")
		// Use 'latest' as all pending transactions are tracked in the tx storage
		const onChainNonce = await this.#publicClient.getTransactionCount({
			address: this.#account.address,
			blockTag: "latest",
		});

		// Estimate gas if not explicitly provided by the caller
		const gas =
			txData.gas ??
			(await this.#publicClient.estimateGas({
				account: this.#account.address,
				to: txData.to,
				data: txData.data,
				value: txData.value,
			}));

		// 2. Reserve Nonce & Persist Intent (Atomic DB Operation)
		// This calculates the correct nonce and saves the record as 'QUEUED'
		const txDataWithGas = { ...txData, gas };
		const nonce = this.#txStorage.register(txDataWithGas, onChainNonce);
		try {
			const hash = await this.submitTransaction({ ...txDataWithGas, nonce, fees: null });
			return { nonce, hash };
		} catch (err) {
			// Check if this tx is still the latest, if so delete it and throw error
			// Retrying should happen on action level, which allows timeouts to apply
			if (nonce === this.#txStorage.maxNonce()) {
				this.#txStorage.delete(nonce);
				throw err;
			}

			// If another action was already submitted it is important to prevent potential unused nonces
			// In this case the transaction is kept and retried until executed (to use up the nonce)
			// No error is thrown to avoid that the action is retried.
			return { nonce, hash: null };
		}
	}
}
