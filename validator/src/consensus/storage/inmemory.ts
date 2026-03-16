import { type Address, encodePacked, type Hex, keccak256 } from "viem";
import type { FrostPoint, GroupId, SignatureId } from "../../frost/types.js";
import type { NonceTree, PublicNonceCommitments } from "../signing/nonces.js";
import type { GroupInfoStorage, KeyGenInfoStorage, SignatureRequestStorage } from "./types.js";

type GroupInfo = {
	groupId: GroupId;
	participants: readonly Address[];
	threshold: number;
	verificationShare?: FrostPoint;
	groupPublicKey?: FrostPoint;
	signingShare?: bigint;
};

type KeyGenInfo = {
	encryptionSecretKey: bigint;
	encryptionPublicKeys: Map<Address, FrostPoint>;
	coefficients: readonly bigint[];
	commitments: Map<Address, readonly FrostPoint[]>;
	secretShares: Map<Address, bigint>;
};

type SignatureRequest = {
	sequence: bigint;
	groupId: Hex;
	message: Hex;
	signerNonceCommitments: Map<Address, PublicNonceCommitments>;
	signers: Address[];
};

export class InMemoryClientStorage implements KeyGenInfoStorage, GroupInfoStorage, SignatureRequestStorage {
	#account: Address;
	#keyGenInfo = new Map<GroupId, KeyGenInfo>();
	#groupInfo = new Map<GroupId, GroupInfo>();
	#nonceTrees = new Map<Hex, NonceTree>();
	#chunkNonces = new Map<Hex, Hex>();
	#signatureRequests = new Map<SignatureId, SignatureRequest>();

	constructor(account: Address) {
		this.#account = account;
	}

	private keyGenInfo(groupId: GroupId): KeyGenInfo {
		const info = this.#keyGenInfo.get(groupId);
		if (info === undefined) throw new Error(`No keygen info for group ${groupId}!`);
		return info;
	}

	private groupInfo(groupId: GroupId): GroupInfo {
		const info = this.#groupInfo.get(groupId);
		if (info === undefined) throw new Error(`Unknown group ${groupId}!`);
		return info;
	}

	private checkInformationComplete(participants: readonly Address[], information: Map<Address, unknown>): boolean {
		for (const address of participants) {
			if (!information.has(address)) {
				return false;
			}
		}
		return true;
	}

	accountAddress(): Address {
		return this.#account;
	}

	registerKeyGen(groupId: GroupId, encryptionSecretKey: bigint, coefficients: readonly bigint[]): void {
		// Check if group is known, otherwise this will throw
		this.groupInfo(groupId);
		if (this.#keyGenInfo.has(groupId)) throw new Error(`KeyGen for ${groupId} already registered!`);
		this.#keyGenInfo.set(groupId, {
			encryptionSecretKey,
			encryptionPublicKeys: new Map(),
			coefficients,
			commitments: new Map(),
			secretShares: new Map(),
		});
	}
	registerCommitments(
		groupId: GroupId,
		participant: Address,
		encryptionPublicKey: FrostPoint,
		commitments: readonly FrostPoint[],
	): void {
		const info = this.keyGenInfo(groupId);
		if (info.commitments.has(participant))
			throw new Error(`Commitments for ${groupId}:${participant} already registered!`);
		info.commitments.set(participant, commitments);
		info.encryptionPublicKeys.set(participant, encryptionPublicKey);
	}
	registerSecretShare(groupId: GroupId, participant: Address, share: bigint): void {
		const info = this.keyGenInfo(groupId);
		if (info.secretShares.has(participant))
			throw new Error(`Secret share for ${groupId}:${participant} already registered!`);
		info.secretShares.set(participant, share);
	}
	checkIfCommitmentsComplete(groupId: GroupId): boolean {
		const participants = this.participants(groupId);
		const info = this.keyGenInfo(groupId);
		return this.checkInformationComplete(participants, info.commitments);
	}
	missingCommitments(groupId: GroupId): Address[] {
		const missing: Address[] = [];
		const participants = this.participants(groupId);
		const info = this.keyGenInfo(groupId);
		for (const address of participants) {
			if (!info.commitments.has(address)) {
				missing.push(address);
			}
		}
		return missing;
	}
	checkIfSecretSharesComplete(groupId: GroupId): boolean {
		const participants = this.participants(groupId);
		const info = this.keyGenInfo(groupId);
		return this.checkInformationComplete(participants, info.secretShares);
	}
	missingSecretShares(groupId: GroupId): Address[] {
		const missing: Address[] = [];
		const participants = this.participants(groupId);
		const info = this.keyGenInfo(groupId);
		for (const address of participants) {
			if (!info.secretShares.has(address)) {
				missing.push(address);
			}
		}
		return missing;
	}
	encryptionSecretKey(groupId: GroupId): bigint {
		const info = this.keyGenInfo(groupId);
		return info.encryptionSecretKey;
	}
	encryptionPublicKey(groupId: GroupId, participant: Address): FrostPoint {
		const info = this.keyGenInfo(groupId);
		const encryptionPublicKey = info.encryptionPublicKeys.get(participant);
		if (encryptionPublicKey === undefined) throw new Error(`No encryption public key for ${participant} available!`);
		return encryptionPublicKey;
	}
	coefficients(groupId: GroupId): readonly bigint[] {
		const info = this.keyGenInfo(groupId);
		return info.coefficients;
	}
	commitments(groupId: GroupId, participant: Address): readonly FrostPoint[] {
		const info = this.keyGenInfo(groupId);
		const commitments = info.commitments.get(participant);
		if (commitments === undefined) throw new Error(`No commitments for ${participant} available!`);
		return commitments;
	}
	commitmentsMap(groupId: GroupId): Map<Address, readonly FrostPoint[]> {
		const info = this.keyGenInfo(groupId);
		return info.commitments;
	}
	secretSharesMap(groupId: GroupId): Map<Address, bigint> {
		const info = this.keyGenInfo(groupId);
		return info.secretShares;
	}
	clearKeyGen(groupId: GroupId): void {
		this.#keyGenInfo.delete(groupId);
	}
	knownGroups(): GroupId[] {
		return Array.from(this.#groupInfo.values().map((g) => g.groupId));
	}
	registerGroup(groupId: GroupId, participants: readonly Address[], threshold: number): Address {
		if (this.#groupInfo.has(groupId)) throw new Error(`Group ${groupId} already registered!`);
		if (!participants.includes(this.#account)) throw new Error(`Not part of Group ${groupId}!`);
		this.#groupInfo.set(groupId, {
			groupId,
			participants,
			threshold,
		});
		return this.#account;
	}
	registerVerification(groupId: GroupId, groupPublicKey: FrostPoint, verificationShare: FrostPoint): void {
		const info = this.groupInfo(groupId);
		if (info.groupPublicKey !== undefined || info.verificationShare !== undefined)
			throw new Error(`Verification information for ${groupId} already set!`);
		info.groupPublicKey = groupPublicKey;
		info.verificationShare = verificationShare;
	}
	registerSigningShare(groupId: GroupId, signingShare: bigint): void {
		const info = this.groupInfo(groupId);
		if (info.signingShare !== undefined) throw new Error(`Signing share for ${groupId} already set!`);
		info.signingShare = signingShare;
	}
	participant(groupId: GroupId): Address {
		if (!this.groupInfo(groupId).participants.includes(this.#account))
			throw new Error(`Not participating in ${groupId}!`);
		return this.#account;
	}
	publicKey(groupId: GroupId): FrostPoint | undefined {
		return this.groupInfo(groupId).groupPublicKey;
	}
	participants(groupId: GroupId): readonly Address[] {
		return this.groupInfo(groupId).participants;
	}
	threshold(groupId: GroupId): number {
		return this.groupInfo(groupId).threshold;
	}
	signingShare(groupId: GroupId): bigint | undefined {
		return this.groupInfo(groupId).signingShare;
	}
	verificationShare(groupId: GroupId): FrostPoint {
		const verificationShare = this.groupInfo(groupId).verificationShare;
		if (verificationShare === undefined) throw new Error(`Verificatrion share not available for ${groupId}!`);
		return verificationShare;
	}
	unregisterGroup(groupId: GroupId): void {
		this.#groupInfo.delete(groupId);
	}

	/*
	 * Signing related storage
	 */

	private signatureRequest(signatureId: SignatureId): SignatureRequest {
		const request = this.#signatureRequests.get(signatureId);
		if (request === undefined) throw new Error(`Unknown signature request ${signatureId}!`);
		return request;
	}

	registerSignatureRequest(
		signatureId: SignatureId,
		groupId: GroupId,
		message: Hex,
		signers: Address[],
		sequence: bigint,
	): void {
		// Check if group is known, otherwise this will throw
		this.groupInfo(groupId);
		if (this.#signatureRequests.has(signatureId))
			throw new Error(`SignatureRequest for ${signatureId} already registered!`);
		this.#signatureRequests.set(signatureId, {
			groupId,
			message,
			signerNonceCommitments: new Map(),
			signers,
			sequence,
		});
	}
	registerNonceCommitments(
		signatureId: SignatureId,
		signerId: Address,
		nonceCommitments: PublicNonceCommitments,
	): void {
		const request = this.signatureRequest(signatureId);
		if (request.signerNonceCommitments.has(signerId))
			throw new Error(`Nonce commitments for ${signatureId}:${signerId} already registered!`);
		request.signerNonceCommitments.set(signerId, nonceCommitments);
	}

	checkIfNoncesComplete(signatureId: SignatureId): boolean {
		const request = this.signatureRequest(signatureId);
		return this.checkInformationComplete(request.signers, request.signerNonceCommitments);
	}
	missingNonces(signatureId: SignatureId): Address[] {
		const missing: Address[] = [];
		const request = this.signatureRequest(signatureId);
		for (const id of request.signers) {
			if (!request.signerNonceCommitments.has(id)) {
				missing.push(id);
			}
		}
		return missing;
	}
	signingGroup(signatureId: SignatureId): GroupId {
		return this.signatureRequest(signatureId).groupId;
	}
	signers(signatureId: SignatureId): Address[] {
		return this.signatureRequest(signatureId).signers;
	}
	message(signatureId: SignatureId): Hex {
		return this.signatureRequest(signatureId).message;
	}
	sequence(signatureId: SignatureId): bigint {
		return this.signatureRequest(signatureId).sequence;
	}
	nonceCommitmentsMap(signatureId: SignatureId): Map<Address, PublicNonceCommitments> {
		return this.signatureRequest(signatureId).signerNonceCommitments;
	}

	registerNonceTree(_groupId: GroupId, tree: NonceTree): Hex {
		this.#nonceTrees.set(tree.root, tree);
		return tree.root;
	}
	linkNonceTree(groupId: GroupId, chunk: bigint, treeHash: Hex): void {
		const chunkId = keccak256(encodePacked(["bytes32", "uint256"], [groupId, chunk]));
		if (this.#chunkNonces.has(chunkId)) throw new Error(`Chunk ${groupId}:${chunk} has already be linked!`);
		this.#chunkNonces.set(chunkId, treeHash);
	}
	nonceTree(groupId: GroupId, chunk: bigint): NonceTree {
		const chunkId = keccak256(encodePacked(["bytes32", "uint256"], [groupId, chunk]));
		const treeHash = this.#chunkNonces.get(chunkId);
		if (treeHash === undefined) throw new Error(`No nonces linked to ${groupId}:${chunk}!`);
		const nonceTree = this.#nonceTrees.get(treeHash);
		if (nonceTree === undefined) throw new Error(`No nonces available for ${groupId}:${chunk}!`);
		return nonceTree;
	}
	burnNonce(groupId: GroupId, chunk: bigint, offset: bigint): void {
		const chunkId = keccak256(encodePacked(["bytes32", "uint256"], [groupId, chunk]));
		const treeHash = this.#chunkNonces.get(chunkId);
		if (treeHash === undefined) throw new Error(`No nonces linked to ${groupId}:${chunk}!`);
		const nonceTree = this.#nonceTrees.get(treeHash);
		if (nonceTree === undefined) throw new Error(`No nonces available for ${groupId}:${chunk}!`);
		const commitments = nonceTree.commitments.at(Number(offset));
		if (commitments === undefined) throw new Error(`No nonces at offset ${offset}!`);
		if (commitments.bindingNonce === 0n && commitments.hidingNonce === 0n)
			throw new Error(`Nonce for offset ${offset} already burned!`);
		commitments.bindingNonce = 0n;
		commitments.hidingNonce = 0n;
	}
}
