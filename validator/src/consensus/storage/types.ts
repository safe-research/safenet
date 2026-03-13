import type { Address, Hex } from "viem";
import type { FrostPoint, GroupId, SignatureId } from "../../frost/types.js";
import type { NonceTree, PublicNonceCommitments } from "../signing/nonces.js";

export type GroupInfoStorage = {
	knownGroups(): GroupId[];
	registerGroup(groupId: GroupId, participants: readonly Address[], threshold: number): Address;
	registerVerification(groupId: GroupId, groupPublicKey: FrostPoint, verificationShare: FrostPoint): void;
	registerSigningShare(groupId: GroupId, signingShare: bigint): void;

	// TODO(nlordell): maybe remove me?
	participant(groupId: GroupId): Address;
	publicKey(groupId: GroupId): FrostPoint | undefined;
	participants(groupId: GroupId): readonly Address[];
	threshold(groupId: GroupId): number;
	signingShare(groupId: GroupId): bigint | undefined;
	verificationShare(groupId: GroupId): FrostPoint;
	unregisterGroup(groupId: GroupId): void;
};

export type KeyGenInfoStorage = {
	registerKeyGen(groupId: GroupId, encryptionSecretKey: bigint, coefficients: readonly bigint[]): void;
	registerCommitments(
		groupId: GroupId,
		participant: Address,
		encryptionPublicKey: FrostPoint,
		commitments: readonly FrostPoint[],
	): void;
	registerSecretShare(groupId: GroupId, participant: Address, share: bigint): void;

	missingCommitments(groupId: GroupId): Address[];
	checkIfCommitmentsComplete(groupId: GroupId): boolean;
	missingSecretShares(groupId: GroupId): Address[];
	checkIfSecretSharesComplete(groupId: GroupId): boolean;

	encryptionSecretKey(groupId: GroupId): bigint;
	encryptionPublicKey(groupId: GroupId, participant: Address): FrostPoint;
	coefficients(groupId: GroupId): readonly bigint[];
	commitments(groupId: GroupId, participant: Address): readonly FrostPoint[];
	commitmentsMap(groupId: GroupId): Map<Address, readonly FrostPoint[]>;
	secretSharesMap(groupId: GroupId): Map<Address, bigint>;
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
