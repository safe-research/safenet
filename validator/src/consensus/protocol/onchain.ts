import type { Address, Hex, PublicClient, WalletClient } from "viem";
import type { FrostPoint, GroupId, SignatureId } from "../../frost/types.js";
import { CONSENSUS_FUNCTIONS, COORDINATOR_FUNCTIONS } from "../../types/abis.js";
import type { Queue } from "../../utils/queue.js";
import { BaseProtocol } from "./base.js";
import type {
	ActionWithTimeout,
	AttestTransaction,
	ConfirmKeyGen,
	PublishSecretShares,
	PublishSignatureShare,
	RegisterNonceCommitments,
	RequestSignature,
	RevealNonceCommitments,
	StageEpoch,
	StartKeyGen,
} from "./types.js";

export class OnchainProtocol extends BaseProtocol {
	#publicClient: PublicClient;
	#signingClient: WalletClient;
	#consensus: Address;
	#coordinator: Address;

	constructor(
		publicClient: PublicClient,
		signingClient: WalletClient,
		consensus: Address,
		coordinator: Address,
		queue: Queue<ActionWithTimeout>,
		logger?: (msg: unknown) => void,
	) {
		super(queue, logger);
		this.#publicClient = publicClient;
		this.#signingClient = signingClient;
		this.#consensus = consensus;
		this.#coordinator = coordinator;
	}
	chainId(): bigint {
		const chainId = this.#signingClient.chain?.id;
		if (chainId === undefined) {
			throw new Error("Unknown chain id");
		}
		return BigInt(chainId);
	}
	consensus(): Address {
		return this.#consensus;
	}
	coordinator(): Address {
		return this.#coordinator;
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
		const { request } = await this.#publicClient.simulateContract({
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
			gas: 250_000n, // TODO: this seems to be wrongly estimated
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}

	protected async publishKeygenSecretShares({ groupId, verificationShare, shares }: PublishSecretShares): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
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
			account: this.#signingClient.account,
			gas: 350_000n,
		});
		return this.#signingClient.writeContract(request);
	}

	private async confirmKeyGenWithCallback(groupId: GroupId, callbackContext: Hex): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
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
			account: this.#signingClient.account,
			gas: 300_000n,
		});
		return this.#signingClient.writeContract(request);
	}

	protected async confirmKeyGen({ groupId, callbackContext }: ConfirmKeyGen): Promise<Hex> {
		if (callbackContext !== undefined) {
			return this.confirmKeyGenWithCallback(groupId, callbackContext);
		}
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenConfirm",
			args: [groupId],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}

	protected async requestSignature({ groupId, message }: RequestSignature): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "sign",
			args: [groupId, message],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}

	protected async registerNonceCommitments({ groupId, nonceCommitmentsHash }: RegisterNonceCommitments): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "preprocess",
			args: [groupId, nonceCommitmentsHash],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}

	protected async revealNonceCommitments({
		signatureId,
		nonceCommitments,
		nonceProof,
	}: RevealNonceCommitments): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
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
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
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
		const { request } = await this.#publicClient.simulateContract({
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
			account: this.#signingClient.account,
			gas: 400_000n, // TODO: this seems to be wrongly estimated
		});
		return this.#signingClient.writeContract(request);
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
		const { request } = await this.#publicClient.simulateContract({
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
			account: this.#signingClient.account,
			gas: 400_000n, // TODO: this seems to be wrongly estimated
		});
		return this.#signingClient.writeContract(request);
	}
	protected async attestTransaction({ epoch, transactionHash, signatureId }: AttestTransaction): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#consensus,
			abi: CONSENSUS_FUNCTIONS,
			functionName: "attestTransaction",
			args: [epoch, transactionHash, signatureId],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}
	protected async stageEpoch({ proposedEpoch, rolloverBlock, groupId, signatureId }: StageEpoch): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#consensus,
			abi: CONSENSUS_FUNCTIONS,
			functionName: "stageEpoch",
			args: [proposedEpoch, rolloverBlock, groupId, signatureId],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}
}
