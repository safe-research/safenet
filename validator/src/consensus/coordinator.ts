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
} from "../frost/types.js";
import type { FrostCoordinator } from "./types.js";

export const COORDINATOR_FUNCTIONS = parseAbi([
	"error InvalidKeyGenCommitment()",
	"error NotParticipating()",
	"function keygenCommit(bytes32 id, uint256 index, bytes32[] calldata poap, ((uint256 x, uint256 y)[] c, (uint256 x, uint256 y) r, uint256 mu) calldata commitment) external",
	"function keygenSecretShare(bytes32 id, uint256 index, ((uint256 x, uint256 y) y, uint256[] f) calldata share) external",
]);

export class OnchainCoordinator implements FrostCoordinator {
	#publicClient: PublicClient;
	#signingClient: WalletClient;
	#address: Address;

	constructor(
		publicClient: PublicClient,
		signingClient: WalletClient,
		address: Address,
	) {
		this.#publicClient = publicClient;
		this.#signingClient = signingClient;
		this.#address = address;
	}

	async publishKeygenCommitments(
		groupId: GroupId,
		index: bigint,
		commits: FrostPoint[],
		pok: ProofOfKnowledge,
		poap: ProofOfAttestationParticipation,
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#address,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keygenCommit",
			args: [
				groupId,
				index,
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
		index: bigint,
		verificationShare: FrostPoint,
		peerShares: bigint[],
	): Promise<Hex> {
		const { request } = await this.#publicClient.simulateContract({
			address: this.#address,
			abi: COORDINATOR_FUNCTIONS,
			functionName: "keygenSecretShare",
			args: [
				groupId,
				index,
				{
					y: verificationShare,
					f: peerShares,
				},
			],
			account: this.#signingClient.account,
		});
		return this.#signingClient.writeContract(request);
	}
}
