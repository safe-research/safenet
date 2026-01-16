import {
	type Account,
	type Address,
	type Chain,
	encodeFunctionData,
	type Hex,
	type PublicClient,
	type SimulateContractParameters,
	TransactionReceiptNotFoundError,
	type Transport,
	type WalletClient,
	zeroHash,
} from "viem";
import type { FrostPoint, GroupId, SignatureId } from "../../frost/types.js";
import { CONSENSUS_FUNCTIONS, COORDINATOR_FUNCTIONS } from "../../types/abis.js";
import type { Logger } from "../../utils/logging.js";
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

export type EthTransactionData = { to: Address; value: bigint; data: Hex; gas?: bigint };

export interface TransactionStorage {
	register(tx: EthTransactionData, minNonce: number): number;
	setHash(nonce: number, txHash: Hex): void;
	setExecuted(nonce: number): void;
	pending(createdDiff: number): (EthTransactionData & { nonce: number; hash: Hex | null })[];
}

export class OnchainProtocol extends BaseProtocol {
	#publicClient: PublicClient;
	#signingClient: WalletClient<Transport, Chain, Account>;
	#txStorage: TransactionStorage;
	#consensus: Address;
	#coordinator: Address;
	#logger: Logger;
	#txStatusPollingSeconds: number;
	#timeBeforeResubmitSeconds: number;

	constructor(
		publicClient: PublicClient,
		signingClient: WalletClient<Transport, Chain, Account>,
		consensus: Address,
		coordinator: Address,
		queue: Queue<ActionWithTimeout>,
		txStorage: TransactionStorage,
		logger: Logger,
		txStatusPollingSeconds = 5,
		timeBeforeResubmitSeconds: number = txStatusPollingSeconds,
	) {
		super(queue, logger);
		this.#publicClient = publicClient;
		this.#signingClient = signingClient;
		this.#txStorage = txStorage;
		this.#consensus = consensus;
		this.#coordinator = coordinator;
		this.#logger = logger;
		this.#txStatusPollingSeconds = txStatusPollingSeconds;
		this.#timeBeforeResubmitSeconds = timeBeforeResubmitSeconds;
		this.checkPending();
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

	private async checkPending() {
		try {
			// We will only mark transaction as executed when get to the point of deciding if we need to resubmit them
			const pendingTxs = this.#txStorage.pending(this.#timeBeforeResubmitSeconds);
			for (const tx of pendingTxs) {
				try {
					// If we don't have a hash we throw an error to trigger resubmission
					if (tx.hash === null) {
						throw new TransactionReceiptNotFoundError({ hash: zeroHash });
					}
					const receipt = await this.#publicClient.getTransactionReceipt({
						hash: tx.hash,
					});
					this.#logger.debug(
						`Transaction with nonce ${tx.nonce} has been executed at block ${receipt.blockNumber}!`,
						tx,
						receipt,
					);
					this.#txStorage.setExecuted(tx.nonce);
					continue;
				} catch (error) {
					// Any other error than transaction not found is unexpected
					if (!(error instanceof TransactionReceiptNotFoundError)) {
						this.#logger.warn(`Unexpected error fetching receipt for ${tx.nonce}!`, { error });
						continue;
					}
				}
				// If we don't find the transaction or it has no blockHash then we resubmit it
				this.#logger.debug(`Resubmit transaction for ${tx.nonce}!`, tx);
				try {
					await this.submitTransaction(tx);
				} catch (error) {
					this.#logger.warn(`Error submitting transaction for ${tx.nonce}!`, { error });
				}
			}
		} catch (error) {
			this.#logger.error("Error while checking pending transactions.", { error });
		} finally {
			setTimeout(() => this.checkPending(), this.#txStatusPollingSeconds * 1000);
		}
	}

	private async submitTransaction(tx: EthTransactionData & { nonce: number }): Promise<Hex> {
		const txHash = await this.#signingClient.sendTransaction({
			...tx,
			chain: this.#signingClient.chain,
			account: this.#signingClient.account,
		});
		this.#txStorage.setHash(tx.nonce, txHash);
		return txHash;
	}

	private async submitAction(action: SimulateContractParameters): Promise<Hex> {
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
		});
	}

	protected async startKeyGen({
		participants,
		count,
		threshold,
		context,
		participantId,
		commitments,
		pok,
		poap,
	}: StartKeyGen): Promise<Hex> {
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

	protected async publishKeygenSecretShares({ groupId, verificationShare, shares }: PublishSecretShares): Promise<Hex> {
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
			gas: 350_000n,
		});
	}

	private async confirmKeyGenWithCallback(groupId: GroupId, callbackContext: Hex): Promise<Hex> {
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

	protected async complain({ groupId, accused }: Complain): Promise<Hex> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenComplain",
			args: [groupId, accused],
		});
	}

	protected async complaintResponse({ groupId, plaintiff, secretShare }: ComplaintResponse): Promise<Hex> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenComplaintResponse",
			args: [groupId, plaintiff, secretShare],
		});
	}

	protected async confirmKeyGen({ groupId, callbackContext }: ConfirmKeyGen): Promise<Hex> {
		if (callbackContext !== undefined) {
			return this.confirmKeyGenWithCallback(groupId, callbackContext);
		}
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenConfirm",
			args: [groupId],
			gas: 100_000n,
		});
	}

	protected async requestSignature({ groupId, message }: RequestSignature): Promise<Hex> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "sign",
			args: [groupId, message],
		});
	}

	protected async registerNonceCommitments({ groupId, nonceCommitmentsHash }: RegisterNonceCommitments): Promise<Hex> {
		return this.submitAction({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "preprocess",
			args: [groupId, nonceCommitmentsHash],
		});
	}

	protected async revealNonceCommitments({
		signatureId,
		nonceCommitments,
		nonceProof,
	}: RevealNonceCommitments): Promise<Hex> {
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
		});
	}

	private async publishSignatureShareWithCallback(
		signatureId: SignatureId,
		signersRoot: Hex,
		signersProof: Hex[],
		groupCommitment: FrostPoint,
		commitmentShare: FrostPoint,
		signatureShare: bigint,
		lagrangeCoefficient: bigint,
		callbackContext: Hex,
	): Promise<Hex> {
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
			gas: 400_000n, // TODO: this seems to be wrongly estimated
		});
	}

	protected async publishSignatureShare({
		signatureId,
		signersRoot,
		signersProof,
		groupCommitment,
		commitmentShare,
		signatureShare,
		lagrangeCoefficient,
		callbackContext,
	}: PublishSignatureShare): Promise<Hex> {
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
			gas: 400_000n, // TODO: this seems to be wrongly estimated
		});
	}

	protected async attestTransaction({ epoch, transactionHash, signatureId }: AttestTransaction): Promise<Hex> {
		return this.submitAction({
			address: this.#consensus,
			abi: CONSENSUS_FUNCTIONS,
			functionName: "attestTransaction",
			args: [epoch, transactionHash, signatureId],
		});
	}

	protected async stageEpoch({ proposedEpoch, rolloverBlock, groupId, signatureId }: StageEpoch): Promise<Hex> {
		return this.submitAction({
			address: this.#consensus,
			abi: CONSENSUS_FUNCTIONS,
			functionName: "stageEpoch",
			args: [proposedEpoch, rolloverBlock, groupId, signatureId],
		});
	}
}
