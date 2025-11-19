import {
	type Address,
	type Hex,
	type PublicClient,
	parseAbi,
	type WalletClient,
} from "viem";
import type {
	FrostPoint,
	GroupId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
	SignatureId,
} from "../frost/types.js";
import type { PublicNonceCommitments } from "./signing/nonces.js";
import type { ShieldnetCoordinator } from "./types.js";

export const COORDINATOR_FUNCTIONS = parseAbi([
	"error AlreadyRegistered()",
	"error InvalidKeyGenCommitment()",
	"error NotParticipating()",
	"function keyGenAndCommit(bytes32 participants, uint64 count, uint64 threshold, bytes32 context, uint256 identifier, bytes32[] poap, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment) external",
	"function keyGenCommit(bytes32 id, uint256 identifier, bytes32[] poap, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) commitment) external",
	"function keyGenSecretShare(bytes32 id, ((uint256 x, uint256 y) y, uint256[] f) share) external",
	"function preprocess(bytes32 id, bytes32 commitment) external returns (uint32 chunk)",
	"function signRevealNonces(bytes32 sid, ((uint256 x, uint256 y) d, (uint256 x, uint256 y) e) nonces, bytes32[] proof) external",
	"function signShare(bytes32 sid, bytes32 root, (uint256 x, uint256 y) r, uint256 z, uint256 cl, bytes32[] proof) external",
]);

export const CONSENSUS_FUNCTIONS = parseAbi([
	"error InvalidRollover()",
	"function proposeEpoch(uint64 proposedEpoch, uint64 rolloverAt, bytes32 group) external",
	"function stageEpoch(uint64 proposedEpoch, uint64 rolloverAt, bytes32 group, ((uint256 x, uint256 y) r, uint256 z) signature) external",
]);

export class OnchainCoordinator implements ShieldnetCoordinator {
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

	async publishKeygenSecretShares(
		groupId: GroupId,
		verificationShare: FrostPoint,
		peerShares: bigint[],
	): Promise<Hex> {
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
		groupCommitementShare: FrostPoint,
		signatureShare: bigint,
		lagrangeChallenge: bigint,
		signingParticipantsProof: Hex[],
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#coordinator,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "signShare",
			args: [
				signatureId,
				signingParticipantsHash,
				groupCommitementShare,
				signatureShare,
				lagrangeChallenge,
				signingParticipantsProof,
			],
			account: this.#signingClient.account,
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
		groupCommitment: FrostPoint,
		groupSignature: bigint,
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#consensus,
			abi: CONSENSUS_FUNCTIONS,
			functionName: "stageEpoch",
			args: [
				proposedEpoch,
				rolloverAt,
				group,
				{
					r: groupCommitment,
					z: groupSignature,
				},
			],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}
}
