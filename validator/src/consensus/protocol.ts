import type { Address, Hex, PublicClient, WalletClient } from "viem";
import type {
	FrostPoint,
	GroupId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
	SignatureId,
} from "../frost/types.js";
import { CONSENSUS_FUNCTIONS, COORDINATOR_FUNCTIONS } from "../types/abis.js";
import type { PublicNonceCommitments } from "./signing/nonces.js";
import type { ShieldnetProtocol } from "./types.js";

export class OnchainProtocol implements ShieldnetProtocol {
	#publicClient: PublicClient;
	#signingClient: WalletClient;
	#consensus: Address;
	#coordinator: Address;

	constructor(
		publicClient: PublicClient,
		signingClient: WalletClient,
		consensus: Address,
		coordinator: Address,
	) {
		this.#publicClient = publicClient;
		this.#signingClient = signingClient;
		this.#consensus = consensus;
		this.#coordinator = coordinator;
	}
	chainId(): bigint {
		const chainId = this.#signingClient.chain?.id;
		if (chainId === undefined) {
			throw Error("Unknown chain id");
		}
		return BigInt(chainId);
	}
	consensus(): Address {
		return this.#consensus;
	}
	coordinator(): Address {
		return this.#consensus;
	}
	async triggerKeygenAndCommit(
		participants: Hex,
		count: bigint,
		threshold: bigint,
		context: Hex,
		id: bigint,
		commits: FrostPoint[],
		pok: ProofOfKnowledge,
		poap: ProofOfAttestationParticipation,
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenAndCommit",
			args: [
				participants,
				count,
				threshold,
				context,
				id,
				poap,
				{
					c: commits,
					r: pok.r,
					mu: pok.mu,
				},
			],
			gas: 250_000n, // TODO: this seems to be wrongly estimated
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}

	async publishKeygenCommitments(
		groupId: GroupId,
		id: bigint,
		commits: FrostPoint[],
		pok: ProofOfKnowledge,
		poap: ProofOfAttestationParticipation,
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenCommit",
			args: [
				groupId,
				id,
				poap,
				{
					c: commits,
					r: pok.r,
					mu: pok.mu,
				},
			],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}

	async publishKeygenSecretSharesWithCallback(
		groupId: GroupId,
		verificationShare: FrostPoint,
		peerShares: bigint[],
		callbackContext: Hex,
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenSecretShareWithCallback",
			args: [
				groupId,
				{
					y: verificationShare,
					f: peerShares,
				},
				{
					target: this.#consensus,
					context: callbackContext,
				},
			],
			account: this.#signingClient.account,
			gas: 300_000n, // TODO: this seems to be wrongly estimated
		});
		return this.#signingClient.writeContract(request);
	}

	async publishKeygenSecretShares(
		groupId: GroupId,
		verificationShare: FrostPoint,
		peerShares: bigint[],
		callbackContext?: Hex,
	): Promise<Hex> {
		// TODO: use callback
		if (callbackContext !== undefined) {
			return this.publishKeygenSecretSharesWithCallback(
				groupId,
				verificationShare,
				peerShares,
				callbackContext,
			);
		}
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keyGenSecretShare",
			args: [
				groupId,
				{
					y: verificationShare,
					f: peerShares,
				},
			],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}

	async publishNonceCommitmentsHash(
		groupId: GroupId,
		nonceCommitmentsHash: Hex,
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "preprocess",
			args: [groupId, nonceCommitmentsHash],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}

	async publishNonceCommitments(
		signatureId: SignatureId,
		nonceCommitments: PublicNonceCommitments,
		nonceProof: Hex[],
	): Promise<Hex> {
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

	async publishSignatureShare(
		signatureId: SignatureId,
		signingParticipantsHash: Hex,
		signingParticipantsProof: Hex[],
		groupCommitement: FrostPoint,
		groupCommitementShare: FrostPoint,
		signatureShare: bigint,
		lagrange: bigint,
	): Promise<Hex> {
		// TODO: use callback
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "signShare",
			args: [
				signatureId,
				{
					r: groupCommitement,
					root: signingParticipantsHash,
				},
				{
					r: groupCommitementShare,
					z: signatureShare,
					l: lagrange,
				},
				signingParticipantsProof,
			],
			account: this.#signingClient.account,
			gas: 400_000n, // TODO: this seems to be wrongly estimated
		});
		return this.#signingClient.writeContract(request);
	}

	async proposeEpoch(
		proposedEpoch: bigint,
		rolloverAt: bigint,
		group: GroupId,
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#consensus,
			abi: CONSENSUS_FUNCTIONS,
			functionName: "proposeEpoch",
			args: [proposedEpoch, rolloverAt, group],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}

	async stageEpoch(
		proposedEpoch: bigint,
		rolloverAt: bigint,
		group: GroupId,
		signature: SignatureId,
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#consensus,
			abi: CONSENSUS_FUNCTIONS,
			functionName: "stageEpoch",
			args: [proposedEpoch, rolloverAt, group, signature],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}
}
