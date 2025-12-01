import type { Address, Hex } from "viem";
import type {
	FrostPoint,
	GroupId,
	ParticipantId,
	SignatureId,
} from "../../frost/types.js";
import type { NonceTree, PublicNonceCommitments } from "../signing/nonces.js";

export type Participant = {
	id: ParticipantId;
	address: Address;
};

export type GroupInfoStorage = {
	knownGroups(): GroupId[];
	registerGroup(
		groupId: GroupId,
		participants: readonly Participant[],
		threshold: bigint,
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
	threshold(groupId: GroupId): bigint;
	signingShare(groupId: GroupId): bigint | undefined;
	verificationShare(groupId: GroupId): FrostPoint;
	unregisterGroup(groupId: GroupId): void;
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

	missingCommitments(groupId: GroupId): ParticipantId[];
	checkIfCommitmentsComplete(groupId: GroupId): boolean;
	missingSecretShares(groupId: GroupId): ParticipantId[];
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
};

export type NonceStorage = {
	registerNonceTree(groupId: GroupId, tree: NonceTree): Hex;
	linkNonceTree(groupId: GroupId, chunk: bigint, treeHash: Hex): void;
	nonceTree(groupId: GroupId, chunk: bigint): NonceTree;
	burnNonce(groupId: GroupId, chunk: bigint, offset: bigint): void;
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
	missingNonces(signatureId: SignatureId): ParticipantId[];

	signingGroup(signatureId: SignatureId): GroupId;
	signers(signatureId: SignatureId): ParticipantId[];
	message(signatureId: SignatureId): Hex;
	sequence(signatureId: SignatureId): bigint;
	nonceCommitmentsMap(
		signatureId: SignatureId,
	): Map<ParticipantId, PublicNonceCommitments>;
} & NonceStorage;
