import { type Address, type Chain, encodeFunctionData, type Hex, type PublicClient, type Transport } from "viem";
import type { FrostPoint, GroupId, SignatureId } from "../../frost/types.js";
import { CONSENSUS_FUNCTIONS, COORDINATOR_FUNCTIONS } from "../../types/abis.js";
import type { ValidatorAccount } from "../../types/account.js";
import type { Logger } from "../../utils/logging.js";
import type { Queue } from "../../utils/queue.js";
import { BaseProtocol, type SubmittedAction } from "./base.js";
import { type GasFeeEstimator, TransactionManager, type TransactionStorage } from "./transaction.js";
import type {
	ActionWithTimeout,
	AttestTransaction,
	Complain,
	ComplaintResponse,
	ConfirmKeyGen,
	DeclineSignature,
	PublishSecretShares,
	PublishSignatureShare,
	RegisterNonceCommitments,
	RequestSignature,
	RevealNonceCommitments,
	SetValidatorStaker,
	StageEpoch,
	StartKeyGen,
} from "./types.js";

export class OnchainProtocol extends BaseProtocol {
	#publicClient: PublicClient<Transport, Chain>;
	#consensus: Address;
	#coordinator: Address;
	#txManager: TransactionManager;

	constructor({
		publicClient,
		account,
		gasFeeEstimator,
		consensus,
		coordinator,
		queue,
		txStorage,
		logger,
		blocksBeforeResubmit,
	}: {
		publicClient: PublicClient<Transport, Chain>;
		account: ValidatorAccount;
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
		this.#consensus = consensus;
		this.#coordinator = coordinator;
		this.#txManager = new TransactionManager({
			publicClient,
			account,
			gasFeeEstimator,
			txStorage,
			logger,
			blocksBeforeResubmit,
		});
	}

	chainId(): bigint {
		const chainId = this.#publicClient.chain.id;
		return BigInt(chainId);
	}

	consensus(): Address {
		return this.#consensus;
	}

	coordinator(): Address {
		return this.#coordinator;
	}

	isRunningPendingCheck(): boolean {
		return this.#txManager.isRunningPendingCheck();
	}

	triggerPendingCheck(blockNumber: bigint) {
		this.#txManager.triggerPendingCheck(blockNumber);
	}

	protected startKeyGen({
		participants,
		count,
		threshold,
		context,
		encryptionPublicKey,
		commitments,
		pok,
		poap,
	}: StartKeyGen): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "keyGenAndCommit",
				args: [
					participants,
					count,
					threshold,
					context,
					poap,
					{ q: encryptionPublicKey, c: commitments, r: pok.r, mu: pok.mu },
				],
			}),
			value: 0n,
			gas: 250_000n,
		});
	}

	protected publishKeygenSecretShares({
		groupId,
		verificationShare,
		shares,
	}: PublishSecretShares): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "keyGenSecretShare",
				args: [groupId, { y: verificationShare, f: shares }],
			}),
			value: 0n,
			gas: 250_000n + BigInt(shares.length) * 25_000n, // TODO: the gas amount per share has not been estimated
		});
	}

	private confirmKeyGenWithCallback(groupId: GroupId, callbackContext: Hex): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "keyGenConfirmWithCallback",
				args: [groupId, { target: this.#consensus, context: callbackContext }],
			}),
			value: 0n,
			gas: 300_000n,
		});
	}

	protected complain({ groupId, accused }: Complain): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "keyGenComplain",
				args: [groupId, accused],
			}),
			value: 0n,
			gas: 300_000n, // TODO: this has not been estimated yet
		});
	}

	protected complaintResponse({ groupId, plaintiff, secretShare }: ComplaintResponse): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "keyGenComplaintResponse",
				args: [groupId, plaintiff, secretShare],
			}),
			value: 0n,
			gas: 300_000n, // TODO: this has not been estimated yet
		});
	}

	protected confirmKeyGen({ groupId, callbackContext }: ConfirmKeyGen): Promise<SubmittedAction> {
		if (callbackContext !== undefined) {
			return this.confirmKeyGenWithCallback(groupId, callbackContext);
		}
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "keyGenConfirm",
				args: [groupId],
			}),
			value: 0n,
			gas: 200_000n,
		});
	}

	protected requestSignature({ groupId, message }: RequestSignature): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "sign",
				args: [groupId, message],
			}),
			value: 0n,
			gas: 400_000n, // TODO: this has not been estimated yet
		});
	}

	protected registerNonceCommitments({
		groupId,
		nonceCommitmentsHash,
	}: RegisterNonceCommitments): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "preprocess",
				args: [groupId, nonceCommitmentsHash],
			}),
			value: 0n,
			gas: 250_000n,
		});
	}

	protected revealNonceCommitments({
		signatureId,
		nonceCommitments,
		nonceProof,
	}: RevealNonceCommitments): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "signRevealNonces",
				args: [
					signatureId,
					{ d: nonceCommitments.hidingNonceCommitment, e: nonceCommitments.bindingNonceCommitment },
					nonceProof,
				],
			}),
			value: 0n,
			gas: 200_000n,
		});
	}

	protected declineSignature({ signatureId }: DeclineSignature): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "signDecline",
				args: [signatureId],
			}),
			value: 0n,
			gas: 80_000n,
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
	): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "signShareWithCallback",
				args: [
					signatureId,
					{ r: groupCommitment, root: signersRoot },
					{ r: commitmentShare, z: signatureShare, l: lagrangeCoefficient },
					signersProof,
					{ target: this.#consensus, context: callbackContext },
				],
			}),
			value: 0n,
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
	}: PublishSignatureShare): Promise<SubmittedAction> {
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
		return this.#txManager.submitAction({
			to: this.#coordinator,
			data: encodeFunctionData({
				abi: COORDINATOR_FUNCTIONS,
				functionName: "signShare",
				args: [
					signatureId,
					{ r: groupCommitment, root: signersRoot },
					{ r: commitmentShare, z: signatureShare, l: lagrangeCoefficient },
					signersProof,
				],
			}),
			value: 0n,
			gas: 400_000n,
		});
	}

	protected attestTransaction({
		epoch,
		chainId,
		safe,
		safeTxStructHash,
		signatureId,
	}: AttestTransaction): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#consensus,
			data: encodeFunctionData({
				abi: CONSENSUS_FUNCTIONS,
				functionName: "attestTransaction",
				args: [epoch, chainId, safe, safeTxStructHash, signatureId],
			}),
			value: 0n,
			gas: 400_000n, // TODO: this has not been estimated yet
		});
	}

	protected stageEpoch({ proposedEpoch, rolloverBlock, groupId, signatureId }: StageEpoch): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#consensus,
			data: encodeFunctionData({
				abi: CONSENSUS_FUNCTIONS,
				functionName: "stageEpoch",
				args: [proposedEpoch, rolloverBlock, groupId, signatureId],
			}),
			value: 0n,
			gas: 400_000n, // TODO: this has not been estimated yet
		});
	}

	protected setValidatorStaker({ staker }: SetValidatorStaker): Promise<SubmittedAction> {
		return this.#txManager.submitAction({
			to: this.#consensus,
			data: encodeFunctionData({
				abi: CONSENSUS_FUNCTIONS,
				functionName: "setValidatorStaker",
				args: [staker],
			}),
			value: 0n,
			gas: 400_000n, // TODO: this has not been estimated yet
		});
	}
}
