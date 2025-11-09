import type { Address } from "viem";
import type {
	FrostPoint,
	GroupId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
} from "../frost/types.js";

export type Participant = {
	index: bigint;
	address: Address;
};

export type FrostCoordinator = {
	publishKeygenCommitments(
		groupId: GroupId,
		index: bigint,
		commits: FrostPoint[],
		pok: ProofOfKnowledge,
		poap: ProofOfAttestationParticipation,
	): void;

	publishKeygenSecretShares(
		groupId: GroupId,
		index: bigint,
		verificationShare: FrostPoint,
		peerShares: bigint[],
	): void;
};
