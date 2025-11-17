import type { Address, Hex } from "viem";
import type {
	FrostPoint,
	GroupId,
	ParticipantId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
	SignatureId,
} from "../frost/types.js";
import type { NonceTree, PublicNonceCommitments } from "./signing/nonces.js";

export type Participant = {
	id: ParticipantId;
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
		verificationShare: FrostPoint,
		peerShares: bigint[],
	): Promise<Hex>;
};

export type SigningCoordinator = {
	publishNonceCommitmentsHash(
		groupId: GroupId,
		nonceCommitmentsHash: Hex,
	): Promise<Hex>;

	publishNonceCommitments(
		signatureId: SignatureId,
		nonceCommitments: PublicNonceCommitments,
		nonceProof: Hex[],
	): Promise<Hex>;

	publishSignatureShare(
		signatureId: SignatureId,
		signingParticipantsHash: Hex,
		groupCommitementShare: FrostPoint, // add(d, mul(bindingFactor, e)
		signatureShare: bigint,
		lagrangeChallenge: bigint,
		signingParticipantsProof: Hex[],
	): Promise<Hex>;
};

export type FrostCoordinator = KeyGenCoordinator;

export type GroupInfoStorage = {
	knownGroups(): GroupId[];
	registerGroup(
		groupId: GroupId,
		participants: readonly Participant[],
	): ParticipantId;
	registerVerification(
		groupId: GroupId,
		groupPublicKey: FrostPoint,
		verificationShare: FrostPoint,
	): void;
	registerSigningShare(groupId: GroupId, signingShare: bigint): void;

	participantId(groupId: GroupId): ParticipantId;
	publicKey(groupId: GroupId): FrostPoint | undefined;
	participants(groupId: GroupId): readonly Participant[];
	signingShare(groupId: GroupId): bigint | undefined;
	verificationShare(groupId: GroupId): FrostPoint;
	unregisterGroup(groupId: GroupId): void;
};

export type ParticipantStorage = {
	registerParticipants(participants: readonly Participant[]): Hex;
	loadParticipants(hash: Hex): readonly Participant[];
};

export type KeyGenInfoStorage = {
	registerKeyGen(groupId: GroupId, coefficients: readonly bigint[]): void;
	registerCommitments(
		groupId: GroupId,
		participantId: ParticipantId,
		commitments: readonly FrostPoint[],
	): void;
	registerSecretShare(
		groupId: GroupId,
		participantId: ParticipantId,
		share: bigint,
	): void;

	checkIfCommitmentsComplete(groupId: GroupId): boolean;
	checkIfSecretSharesComplete(groupId: GroupId): boolean;

	encryptionKey(groupId: GroupId): bigint;
	coefficients(groupId: GroupId): readonly bigint[];
	commitments(
		groupId: GroupId,
		participantId: ParticipantId,
	): readonly FrostPoint[];
	commitmentsMap(groupId: GroupId): Map<ParticipantId, readonly FrostPoint[]>;
	secretSharesMap(groupId: GroupId): Map<ParticipantId, bigint>;
	clearKeyGen(groupId: GroupId): void;
} & ParticipantStorage;

export type NonceStorage = {
	registerNonceTree(tree: NonceTree): Hex;
	linkNonceTree(groupId: GroupId, chunk: bigint, treeHash: Hex): void;
	nonceTree(groupId: GroupId, chunk: bigint): NonceTree;
};

export type SignatureRequestStorage = {
	registerSignatureRequest(
		signatureId: SignatureId,
		groupId: GroupId,
		message: Hex,
		signers: ParticipantId[],
		sequence: bigint,
	): void;
	registerNonceCommitments(
		signatureId: SignatureId,
		signerId: ParticipantId,
		nonceCommitments: PublicNonceCommitments,
	): void;

	checkIfNoncesComplete(signatureId: SignatureId): boolean;

	signingGroup(signatureId: SignatureId): GroupId;
	signers(signatureId: SignatureId): ParticipantId[];
	message(signatureId: SignatureId): Hex;
	sequence(signatureId: SignatureId): bigint;
	nonceCommitmentsMap(
		signatureId: SignatureId,
	): Map<ParticipantId, PublicNonceCommitments>;
} & NonceStorage;
