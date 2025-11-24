import { type Address, encodePacked, type Hex, keccak256 } from "viem";
import type {
	FrostPoint,
	GroupId,
	ParticipantId,
	SignatureId,
} from "../frost/types.js";
import { calculateParticipantsRoot } from "./merkle.js";
import type { NonceTree, PublicNonceCommitments } from "./signing/nonces.js";
import type {
	GroupInfoStorage,
	KeyGenInfoStorage,
	Participant,
	SignatureRequestStorage,
} from "./types.js";

type GroupInfo = {
	groupId: GroupId;
	participants: readonly Participant[];
	participantId: bigint;
	threshold: bigint;
	verificationShare?: FrostPoint;
	groupPublicKey?: FrostPoint;
	signingShare?: bigint;
};

type KeyGenInfo = {
	coefficients: readonly bigint[];
	commitments: Map<ParticipantId, readonly FrostPoint[]>;
	secretShares: Map<ParticipantId, bigint>;
};

type SignatureRequest = {
	sequence: bigint;
	groupId: Hex;
	message: Hex;
	signerNonceCommitments: Map<ParticipantId, PublicNonceCommitments>;
	signers: ParticipantId[];
};

export class InMemoryStorage
	implements KeyGenInfoStorage, GroupInfoStorage, SignatureRequestStorage
{
	#account: Address;
	#participantsInfo = new Map<Hex, readonly Participant[]>();
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
		if (info === undefined) throw Error(`No keygen info for group ${groupId}!`);
		return info;
	}

	private groupInfo(groupId: GroupId): GroupInfo {
		const info = this.#groupInfo.get(groupId);
		if (info === undefined) throw Error(`Unknown group ${groupId}!`);
		return info;
	}

	private checkInformationComplete(
		participants: readonly ParticipantId[],
		information: Map<bigint, unknown>,
	): boolean {
		for (const id of participants) {
			if (!information.has(id)) {
				return false;
			}
		}
		return true;
	}

	accountAddress(): Address {
		return this.#account;
	}

	registerKeyGen(groupId: GroupId, coefficients: readonly bigint[]): void {
		// Check if group is known, otherwise this will throw
		this.groupInfo(groupId);
		if (this.#keyGenInfo.has(groupId))
			throw Error(`KeyGen for ${groupId} already registered!`);
		this.#keyGenInfo.set(groupId, {
			coefficients,
			commitments: new Map(),
			secretShares: new Map(),
		});
	}
	registerCommitments(
		groupId: GroupId,
		participantId: ParticipantId,
		commitments: readonly FrostPoint[],
	): void {
		const info = this.keyGenInfo(groupId);
		if (info.commitments.has(participantId))
			throw Error(
				`Commitments for ${groupId}:${participantId} already registered!`,
			);
		info.commitments.set(participantId, commitments);
	}
	registerSecretShare(
		groupId: GroupId,
		participantId: ParticipantId,
		share: bigint,
	): void {
		const info = this.keyGenInfo(groupId);
		if (info.secretShares.has(participantId))
			throw Error(
				`Secret share for ${groupId}:${participantId} already registered!`,
			);
		info.secretShares.set(participantId, share);
	}
	checkIfCommitmentsComplete(groupId: GroupId): boolean {
		const participants = this.participants(groupId);
		const info = this.keyGenInfo(groupId);
		return this.checkInformationComplete(
			participants.map((p) => p.id),
			info.commitments,
		);
	}
	checkIfSecretSharesComplete(groupId: GroupId): boolean {
		const participants = this.participants(groupId);
		const info = this.keyGenInfo(groupId);
		return this.checkInformationComplete(
			participants.map((p) => p.id),
			info.secretShares,
		);
	}
	encryptionKey(groupId: GroupId): bigint {
		const info = this.keyGenInfo(groupId);
		return info.coefficients[0];
	}
	coefficients(groupId: GroupId): readonly bigint[] {
		const info = this.keyGenInfo(groupId);
		return info.coefficients;
	}
	commitments(
		groupId: GroupId,
		participantId: ParticipantId,
	): readonly FrostPoint[] {
		const info = this.keyGenInfo(groupId);
		const commitments = info.commitments.get(participantId);
		if (commitments === undefined)
			throw Error(`No commitments for ${participantId} available!`);
		return commitments;
	}
	commitmentsMap(groupId: GroupId): Map<ParticipantId, readonly FrostPoint[]> {
		const info = this.keyGenInfo(groupId);
		return info.commitments;
	}
	secretSharesMap(groupId: GroupId): Map<ParticipantId, bigint> {
		const info = this.keyGenInfo(groupId);
		return info.secretShares;
	}
	clearKeyGen(groupId: GroupId): void {
		this.#keyGenInfo.delete(groupId);
	}
	registerParticipants(participants: readonly Participant[]): Hex {
		const participantsHash = calculateParticipantsRoot(participants);
		this.#participantsInfo.set(participantsHash, participants);
		return participantsHash;
	}
	loadParticipants(hash: Hex): readonly Participant[] {
		const participants = this.#participantsInfo.get(hash);
		if (participants === undefined)
			throw Error(`Unknown participants hash ${hash}!`);
		return participants;
	}
	knownGroups(): GroupId[] {
		return Array.from(this.#groupInfo.values().map((g) => g.groupId));
	}
	registerGroup(
		groupId: GroupId,
		participants: readonly Participant[],
		threshold: bigint,
	): ParticipantId {
		if (this.#groupInfo.has(groupId))
			throw Error(`Group ${groupId} already registered!`);
		const participantId = participants.find(
			(p) => p.address === this.#account,
		)?.id;
		if (participantId === undefined)
			throw Error(`Not part of Group ${groupId}!`);
		this.#groupInfo.set(groupId, {
			participantId,
			groupId,
			participants,
			threshold,
		});
		return participantId;
	}
	registerVerification(
		groupId: GroupId,
		groupPublicKey: FrostPoint,
		verificationShare: FrostPoint,
	): void {
		const info = this.groupInfo(groupId);
		if (
			info.groupPublicKey !== undefined ||
			info.verificationShare !== undefined
		)
			throw Error(`Verification information for ${groupId} already set!`);
		info.groupPublicKey = groupPublicKey;
		info.verificationShare = verificationShare;
	}
	registerSigningShare(groupId: GroupId, signingShare: bigint): void {
		const info = this.groupInfo(groupId);
		if (info.signingShare !== undefined)
			throw Error(`Signing share for ${groupId} already set!`);
		info.signingShare = signingShare;
	}
	participantId(groupId: GroupId): ParticipantId {
		return this.groupInfo(groupId).participantId;
	}
	publicKey(groupId: GroupId): FrostPoint | undefined {
		return this.groupInfo(groupId).groupPublicKey;
	}
	participants(groupId: GroupId): readonly Participant[] {
		return this.groupInfo(groupId).participants;
	}
	threshold(groupId: GroupId): bigint {
		return this.groupInfo(groupId).threshold;
	}
	signingShare(groupId: GroupId): bigint | undefined {
		return this.groupInfo(groupId).signingShare;
	}
	verificationShare(groupId: GroupId): FrostPoint {
		const verificationShare = this.groupInfo(groupId).verificationShare;
		if (verificationShare === undefined)
			throw Error(`Verificatrion share not available for ${groupId}!`);
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
		if (request === undefined)
			throw Error(`Unknown signature request ${signatureId}!`);
		return request;
	}

	registerSignatureRequest(
		signatureId: SignatureId,
		groupId: GroupId,
		message: Hex,
		signers: ParticipantId[],
		sequence: bigint,
	): void {
		// Check if group is known, otherwise this will throw
		this.groupInfo(groupId);
		if (this.#signatureRequests.has(signatureId))
			throw Error(`SignatureRequest for ${signatureId} already registered!`);
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
		signerId: ParticipantId,
		nonceCommitments: PublicNonceCommitments,
	): void {
		const request = this.signatureRequest(signatureId);
		if (request.signerNonceCommitments.has(signerId))
			throw Error(
				`Nonce commitments for ${signatureId}:${signerId} already registered!`,
			);
		request.signerNonceCommitments.set(signerId, nonceCommitments);
	}

	checkIfNoncesComplete(signatureId: SignatureId): boolean {
		const request = this.signatureRequest(signatureId);
		return this.checkInformationComplete(
			request.signers,
			request.signerNonceCommitments,
		);
	}
	signingGroup(signatureId: SignatureId): GroupId {
		return this.signatureRequest(signatureId).groupId;
	}
	signers(signatureId: SignatureId): ParticipantId[] {
		return this.signatureRequest(signatureId).signers;
	}
	message(signatureId: SignatureId): Hex {
		return this.signatureRequest(signatureId).message;
	}
	sequence(signatureId: SignatureId): bigint {
		return this.signatureRequest(signatureId).sequence;
	}
	nonceCommitmentsMap(
		signatureId: SignatureId,
	): Map<ParticipantId, PublicNonceCommitments> {
		return this.signatureRequest(signatureId).signerNonceCommitments;
	}

	registerNonceTree(tree: NonceTree): Hex {
		this.#nonceTrees.set(tree.root, tree);
		return tree.root;
	}
	linkNonceTree(groupId: GroupId, chunk: bigint, treeHash: Hex): void {
		const chunkId = keccak256(
			encodePacked(["bytes32", "uint256"], [groupId, chunk]),
		);
		if (this.#chunkNonces.has(chunkId))
			throw Error(`Chunk ${groupId}:${chunk} has already be linked!`);
		this.#chunkNonces.set(chunkId, treeHash);
	}
	nonceTree(groupId: GroupId, chunk: bigint): NonceTree {
		const chunkId = keccak256(
			encodePacked(["bytes32", "uint256"], [groupId, chunk]),
		);
		const treeHash = this.#chunkNonces.get(chunkId);
		if (treeHash === undefined)
			throw Error(`No nonces linked to ${groupId}:${chunk}!`);
		const nonceTree = this.#nonceTrees.get(treeHash);
		if (nonceTree === undefined)
			throw Error(`No nonces available for ${groupId}:${chunk}!`);
		return nonceTree;
	}
	burnNonce(groupId: GroupId, chunk: bigint, offset: bigint): void {
		const chunkId = keccak256(
			encodePacked(["bytes32", "uint256"], [groupId, chunk]),
		);
		const treeHash = this.#chunkNonces.get(chunkId);
		if (treeHash === undefined)
			throw Error(`No nonces linked to ${groupId}:${chunk}!`);
		const nonceTree = this.#nonceTrees.get(treeHash);
		if (nonceTree === undefined)
			throw Error(`No nonces available for ${groupId}:${chunk}!`);
		const commitments = nonceTree.commitments.at(Number(offset));
		if (commitments === undefined)
			throw Error(`No nonces at offset ${offset}!`);
		if (commitments.bindingNonce === 0n && commitments.hidingNonce === 0n)
			throw Error(`Nonce for offset ${offset} already burned!`);
		commitments.bindingNonce = 0n;
		commitments.hidingNonce = 0n;
	}
}
