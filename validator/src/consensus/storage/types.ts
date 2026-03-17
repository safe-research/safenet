import type { Address, Hex } from "viem";
import type { FrostPoint, GroupId, SignatureId } from "../../frost/types.js";
import type { NonceTree, PublicNonceCommitments } from "../signing/nonces.js";

export type GroupInfoStorage = {
	knownGroups(): GroupId[];
	registerGroup(groupId: GroupId, participants: readonly Address[], threshold: number): void;
	registerGroupKey(groupId: GroupId, groupPublicKey: FrostPoint): void;
	registerVerificationShare(groupId: GroupId, me: Address, verificationShare: FrostPoint): void;
	registerSigningShare(groupId: GroupId, me: Address, signingShare: bigint): void;

	publicKey(groupId: GroupId): FrostPoint | undefined;
	hasParticipant(groupId: GroupId, participant: Address): boolean;
	participants(groupId: GroupId): readonly Address[];
	threshold(groupId: GroupId): number;
	signingShare(groupId: GroupId, me: Address): bigint | undefined;
	verificationShare(groupId: GroupId, me: Address): FrostPoint;
	unregisterGroup(groupId: GroupId): void;
};

export type KeyGenInfoStorage = {
	registerKeyGen(
		groupId: GroupId,
		me: Address,
		encryptionSecretKey: bigint,
		coefficients: readonly bigint[],
	): void;
	registerCommitments(
		groupId: GroupId,
		peer: Address,
		encryptionPublicKey: FrostPoint,
		commitments: readonly FrostPoint[],
	): void;
	registerSecretShare(groupId: GroupId, me: Address, peer: Address, share: bigint): void;

	missingCommitments(groupId: GroupId): Address[];
	checkIfCommitmentsComplete(groupId: GroupId): boolean;
	missingSecretShares(groupId: GroupId, me: Address): Address[];
	checkIfSecretSharesComplete(groupId: GroupId, me: Address): boolean;

	encryptionSecretKey(groupId: GroupId, me: Address): bigint;
	encryptionPublicKey(groupId: GroupId, peer: Address): FrostPoint;
	coefficients(groupId: GroupId, me: Address): readonly bigint[];
	commitments(groupId: GroupId, peer: Address): readonly FrostPoint[];
	commitmentsMap(groupId: GroupId): Map<Address, readonly FrostPoint[]>;
	secretSharesMap(groupId: GroupId, me: Address): Map<Address, bigint>;
	clearKeyGen(groupId: GroupId): void;
};

export type NonceStorage = {
	registerNonceTree(groupId: GroupId, me: Address, tree: NonceTree): Hex;
	linkNonceTree(groupId: GroupId, me: Address, chunk: bigint, treeHash: Hex): void;
	nonceTree(groupId: GroupId, me: Address, chunk: bigint): NonceTree;
	burnNonce(groupId: GroupId, me: Address, chunk: bigint, offset: bigint): void;
};

export type SignatureRequestStorage = {
	registerSignatureRequest(
		signatureId: SignatureId,
		groupId: GroupId,
		message: Hex,
		signers: readonly Address[],
		sequence: bigint,
	): void;
	registerNonceCommitments(signatureId: SignatureId, signer: Address, nonceCommitments: PublicNonceCommitments): void;

	checkIfNoncesComplete(signatureId: SignatureId): boolean;
	missingNonces(signatureId: SignatureId): Address[];

	signingGroup(signatureId: SignatureId): GroupId;
	signers(signatureId: SignatureId): Address[];
	message(signatureId: SignatureId): Hex;
	sequence(signatureId: SignatureId): bigint;
	nonceCommitmentsMap(signatureId: SignatureId): Map<Address, PublicNonceCommitments>;
} & NonceStorage;
