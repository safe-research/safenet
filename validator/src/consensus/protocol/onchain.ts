import {
	type Account,
	type Address,
	type Chain,
	encodeFunctionData,
	type FeeValuesEIP1559,
	type Hex,
	NonceTooLowError,
	type PublicClient,
	type SimulateContractParameters,
	TransactionExecutionError,
	type Transport,
	type WalletClient,
} from "viem";
import type { FrostPoint, GroupId, SignatureId } from "../../frost/types.js";
import { CONSENSUS_FUNCTIONS, COORDINATOR_FUNCTIONS } from "../../types/abis.js";
import type { Logger } from "../../utils/logging.js";
import { maxBigInt } from "../../utils/math.js";
import type { Queue } from "../../utils/queue.js";
import { BaseProtocol } from "./base.js";
import type {
	ActionWithTimeout,
	AttestTransaction,
	Complain,
	ComplaintResponse,
	ConfirmKeyGen,
	PublishSecretShares,
	PublishSignatureShare,
	RegisterNonceCommitments,
	RequestSignature,
	RevealNonceCommitments,
	StageEpoch,
	StartKeyGen,
} from "./types.js";

export type FeeValues = Pick<FeeValuesEIP1559, "maxFeePerGas" | "maxPriorityFeePerGas">;
export type EthTransactionData = { to: Address; value: bigint; data: Hex; gas?: bigint };
export type EthTransactionDetails = { nonce: number; fees: FeeValues | null; hash: Hex | null };

export interface TransactionStorage {
	register(tx: EthTransactionData, minNonce: number): number;
	delete(nonce: number): void;
	setFees(nonce: number, fees: FeeValues): void;
	setHash(nonce: number, txHash: Hex): void;
	setExecuted(nonce: number): void;
	setAllBeforeAsExecuted(nonce: number): number;
	setSubmittedForPending(blockNumber: bigint): number;
	maxNonce(): number | null;
	submittedUpTo(blockNumber: bigint, offset?: number, limit?: number): (EthTransactionData & EthTransactionDetails)[];
}

export class GasFeeEstimator {
	#cachedPrices: Promise<FeeValues> | null = null;
	#client: PublicClient;

	constructor(client: PublicClient) {
		this.#client = client;
	}

	invalidate() {
		this.#cachedPrices = null;
	}

	estimateFees(): Promise<FeeValues> {
		if (this.#cachedPrices !== null) {
			return this.#cachedPrices;
		}
		// Also cache errors, to prevent that on error too many request are fired
		const pricePromise = this.#client.estimateFeesPerGas();
		this.#cachedPrices = pricePromise;
		return pricePromise;
	}
}

export class OnchainProtocol extends BaseProtocol {
	#publicClient: PublicClient;
	#signingClient: WalletClient<Transport, Chain, Account>;
	#gasFeeEstimator: GasFeeEstimator;
	#txStorage: TransactionStorage;
	#consensus: Address;
	#coordinator: Address;
	#logger: Logger;
	#blocksBeforeResubmit: bigint;

	constructor({
		publicClient,
		signingClient,
		gasFeeEstimator,
		consensus,
		coordinator,
		queue,
		txStorage,
		logger,
		blocksBeforeResubmit,
	}: {
		publicClient: PublicClient;
		signingClient: WalletClient<Transport, Chain, Account>;
		gasFeeEstimator: GasFeeEstimator;
		consensus: Address;
		coordinator: Address;
		queue: Queue<ActionWithTimeout>;
		txStorage: TransactionStorage;
		logger: Logger;
		blocksBeforeResubmit?: bigint;
	}) {
		super(queue, logger);
		this.#publicClient = publicClient;
		this.#signingClient = signingClient;
		this.#gasFeeEstimator = gasFeeEstimator;
		this.#txStorage = txStorage;
		this.#consensus = consensus;
		this.#coordinator = coordinator;
		this.#logger = logger;
		this.#blocksBeforeResubmit = blocksBeforeResubmit ?? 1n;
	}

	chainId(): bigint {
		const chainId = this.#signingClient.chain.id;
		return BigInt(chainId);
	}

	consensus(): Address {
		return this.#consensus;
	}

	coordinator(): Address {
		return this.#coordinator;
	}

	async checkPendingActions(blockNumber: bigint) {
		try {
			// For transaction without a submission block set it to this block
			// This assumes that the transaction should be included in this block
			// If the blocksBeforeResubmit is 1 block, these transactions will only be retried on the next block
			const newPendingTxs = this.#txStorage.setSubmittedForPending(blockNumber);
			if (newPendingTxs > 0) {
				this.#logger.debug(`Marked ${newPendingTxs} transactions as submitted at block ${blockNumber}`);
			}
			const currentNonce = await this.#publicClient.getTransactionCount({
				address: this.#signingClient.account.address,
				blockTag: "latest",
			});
			const executedTxs = this.#txStorage.setAllBeforeAsExecuted(currentNonce);
			if (executedTxs > 0) {
				this.#logger.debug(`Marked ${executedTxs} transactions as executed`);
			}
			// Only fetch the first page of pending transactions (default limit is 100) to avoid retrying too many transactions at once.
			const pendingTxs = this.#txStorage.submittedUpTo(blockNumber - this.#blocksBeforeResubmit);
			for (const tx of pendingTxs) {
				// If we don't find the transaction or it has no blockHash then we resubmit it
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
						this.#txStorage.setExecuted(tx.nonce);
						continue;
					}
					this.#logger.warn(`Error submitting transaction for ${tx.nonce}!`, { error });
				}
			}
		} catch (error) {
			this.#logger.error("Error while checking pending transactions.", { error });
		}
	}

	private async submitTransaction(
		tx: EthTransactionData & Pick<EthTransactionDetails, "nonce" | "fees">,
	): Promise<Hex> {
		const estimatedFees = await this.#gasFeeEstimator.estimateFees();
		// Use max of (previous fees + 10%) and estimate
		const fees: FeeValues = {
			maxFeePerGas: maxBigInt(estimatedFees.maxFeePerGas, ((tx.fees?.maxFeePerGas ?? 0n) * 110n) / 100n),
			maxPriorityFeePerGas: maxBigInt(
				estimatedFees.maxPriorityFeePerGas,
				((tx.fees?.maxPriorityFeePerGas ?? 0n) * 110n) / 100n,
			),
		};

		// Store fees before submission in case an error occurs
		this.#txStorage.setFees(tx.nonce, fees);
		const txHash = await this.#signingClient.sendTransaction({
			to: tx.to,
			value: tx.value,
			data: tx.data,
			nonce: tx.nonce,
			gas: tx.gas,
			chain: this.#signingClient.chain,
			account: this.#signingClient.account,
			...fees,
		});
		this.#txStorage.setHash(tx.nonce, txHash);
		return txHash;
	}

	private async submitAction(action: SimulateContractParameters): Promise<Hex | null> {
		// 1. Get Network Baseline (The "Minimum Nonce")
		// We use 'pending' to capture what the node knows about the mempool
		const onChainNonce = await this.#publicClient.getTransactionCount({
			address: this.#signingClient.account.address,
			blockTag: "pending",
		});

		const calldata = encodeFunctionData(action);
		// 2. Reserve Nonce & Persist Intent (Atomic DB Operation)
		// This calculates the correct nonce and saves the record as 'QUEUED'
		const txData = {
			to: action.address,
			data: calldata,
			value: 0n,
			gas: action.gas,
		};
		const nonce = this.#txStorage.register(txData, onChainNonce);
		return this.submitTransaction({
			...txData,
			nonce,
			fees: null,
		}).catch((e) => {
			// Check if this tx is still the latest, if so delete it and throw error
			// Retrying should happen on action level, which allows timeouts to apply
			if (nonce === this.#txStorage.maxNonce()) {
				this.#txStorage.delete(nonce);
				throw e;
			}
			// If another action was already submitted it is important to prevent potential unused nonces
			// In this case the transaction is kept and retried until executed (to use up the nonce)
			// No error is thrown to avoid that the action is retried.
			return null;
		});
	}

	protected startKeyGen({
		participants,
		count,
		threshold,
		context,
		participantId,
		commitments,
		pok,
		poap,
	}: StartKeyGen): Promise<Hex | null> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenAndCommit",
			args: [
				participants,
				count,
				threshold,
				context,
				participantId,
				poap,
				{
					c: commitments,
					r: pok.r,
					mu: pok.mu,
				},
			],
			gas: 250_000n,
		});
	}

	protected publishKeygenSecretShares({
		groupId,
		verificationShare,
		shares,
	}: PublishSecretShares): Promise<Hex | null> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenSecretShare",
			args: [
				groupId,
				{
					y: verificationShare,
					f: shares,
				},
			],
			gas: 250_000n + BigInt(shares.length) * 25_000n, // TODO: the gas amount per share has not been estimates
		});
	}

	private confirmKeyGenWithCallback(groupId: GroupId, callbackContext: Hex): Promise<Hex | null> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenConfirmWithCallback",
			args: [
				groupId,
				{
					target: this.#consensus,
					context: callbackContext,
				},
			],
			gas: 300_000n,
		});
	}

	protected complain({ groupId, accused }: Complain): Promise<Hex | null> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenComplain",
			args: [groupId, accused],
			gas: 300_000n, // TODO: this has not been estimated yet
		});
	}

	protected complaintResponse({ groupId, plaintiff, secretShare }: ComplaintResponse): Promise<Hex | null> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenComplaintResponse",
			args: [groupId, plaintiff, secretShare],
			gas: 300_000n, // TODO: this has not been estimated yet
		});
	}

	protected confirmKeyGen({ groupId, callbackContext }: ConfirmKeyGen): Promise<Hex | null> {
		if (callbackContext !== undefined) {
			return this.confirmKeyGenWithCallback(groupId, callbackContext);
		}
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenConfirm",
			args: [groupId],
			gas: 200_000n,
		});
	}

	protected requestSignature({ groupId, message }: RequestSignature): Promise<Hex | null> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "sign",
			args: [groupId, message],
			gas: 400_000n, // TODO: this has not been estimated yet
		});
	}

	protected registerNonceCommitments({ groupId, nonceCommitmentsHash }: RegisterNonceCommitments): Promise<Hex | null> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "preprocess",
			args: [groupId, nonceCommitmentsHash],
			gas: 250_000n,
		});
	}

	protected revealNonceCommitments({
		signatureId,
		nonceCommitments,
		nonceProof,
	}: RevealNonceCommitments): Promise<Hex | null> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "signRevealNonces",
			args: [
				signatureId,
				{
					d: nonceCommitments.hidingNonceCommitment,
					e: nonceCommitments.bindingNonceCommitment,
				},
				nonceProof,
			],
			gas: 200_000n,
		});
	}

	private publishSignatureShareWithCallback(
		signatureId: SignatureId,
		signersRoot: Hex,
		signersProof: Hex[],
		groupCommitment: FrostPoint,
		commitmentShare: FrostPoint,
		signatureShare: bigint,
		lagrangeCoefficient: bigint,
		callbackContext: Hex,
	): Promise<Hex | null> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "signShareWithCallback",
			args: [
				signatureId,
				{
					r: groupCommitment,
					root: signersRoot,
				},
				{
					r: commitmentShare,
					z: signatureShare,
					l: lagrangeCoefficient,
				},
				signersProof,
				{
					target: this.#consensus,
					context: callbackContext,
				},
			],
			gas: 400_000n,
		});
	}

	protected publishSignatureShare({
		signatureId,
		signersRoot,
		signersProof,
		groupCommitment,
		commitmentShare,
		signatureShare,
		lagrangeCoefficient,
		callbackContext,
	}: PublishSignatureShare): Promise<Hex | null> {
		if (callbackContext !== undefined) {
			return this.publishSignatureShareWithCallback(
				signatureId,
				signersRoot,
				signersProof,
				groupCommitment,
				commitmentShare,
				signatureShare,
				lagrangeCoefficient,
				callbackContext,
			);
		}
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "signShare",
			args: [
				signatureId,
				{
					r: groupCommitment,
					root: signersRoot,
				},
				{
					r: commitmentShare,
					z: signatureShare,
					l: lagrangeCoefficient,
				},
				signersProof,
			],
			gas: 400_000n,
		});
	}

	protected attestTransaction({ epoch, transactionHash, signatureId }: AttestTransaction): Promise<Hex | null> {
		return this.submitAction({
			address: this.#consensus,
			abi: CONSENSUS_FUNCTIONS,
			functionName: "attestTransaction",
			args: [epoch, transactionHash, signatureId],
			gas: 400_000n, // TODO: this has not been estimated yet
		});
	}

	protected stageEpoch({ proposedEpoch, rolloverBlock, groupId, signatureId }: StageEpoch): Promise<Hex | null> {
		return this.submitAction({
			address: this.#consensus,
			abi: CONSENSUS_FUNCTIONS,
			functionName: "stageEpoch",
			args: [proposedEpoch, rolloverBlock, groupId, signatureId],
			gas: 400_000n, // TODO: this has not been estimated yet
		});
	}
}
