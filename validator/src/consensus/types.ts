import type { Address, Hex } from "viem";
import type {
	FrostPoint,
	GroupId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
	SignatureId,
} from "../frost/types.js";
import { PublicNonceCommitments } from "./signing/nonces.js";

export type Participant = {
	index: bigint;
	address: Address;
};

export type KeyGenCoordinator = {
	publishKeygenCommitments(
		groupId: GroupId,
		index: bigint,
		commits: FrostPoint[],
		pok: ProofOfKnowledge,
		poap: ProofOfAttestationParticipation,
	): Promise<Hex>;

	publishKeygenSecretShares(
		groupId: GroupId,
		index: bigint,
		verificationShare: FrostPoint,
		peerShares: bigint[],
	): Promise<Hex>;
};

export type SigningCoordinator = {
	publishNonceCommitmentsHash(
		groupId: GroupId,
		nonceCommitmentsHash: Hex
	): Promise<Hex>

	publishNonceCommitments(
		signatureId: SignatureId,
		nonceCommitments: PublicNonceCommitments,
		nonceProof: Hex[]
	): Promise<Hex>

	publishSignatureShare(
		signatureId: SignatureId,
		signingParticipantsHash: Hex,
		groupCommitementShare: FrostPoint, // add(d, mul(bindingFactor, e)
		signatureShare: bigint,
		lagrangeChallange: bigint,
		signingParticipantsProof: Hex[]
	): Promise<Hex>
}

export type FrostCoordinator = KeyGenCoordinator
