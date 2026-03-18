import type { Address, Hex } from "viem";
import { deriveParticipantId, toParticipantIdMap } from "../../frost/identifier.js";
import { createSigningShare, createVerificationShare, evalCommitment, evalPoly, verifyKey } from "../../frost/math.js";
import { ecdh } from "../../frost/secret.js";
import type { FrostPoint, GroupId, ProofOfAttestationParticipation, ProofOfKnowledge } from "../../frost/types.js";
import {
	createCoefficients,
	createCommitments,
	createEncryptionKey,
	createProofOfKnowledge,
	verifyCommitments,
} from "../../frost/vss.js";
import type { Logger } from "../../utils/logging.js";
import { calculateParticipantsRoot, generateParticipantProof } from "../merkle.js";
import type { GroupInfoStorage, KeyGenInfoStorage } from "../storage/types.js";
import { calcGroupId } from "./utils.js";

export type KeygenInfo = {
	groupId: GroupId;
	participants: Address[];
	coefficients: bigint[];
	commitments: Map<bigint, readonly FrostPoint[]>;
	secretShares: Map<bigint, bigint>;
	verificationShare?: FrostPoint;
	groupPublicKey?: FrostPoint;
	signingShare?: bigint;
};

/**
 * The following order must always strictly kept:
 * 1. register participants root
 * 2. pre keygen
 * 3. publish commitments to other participants
 *   a. receive commitments from other participants
 * 4. publish secret shares
 *   a. receive secret shares
 */
export class KeyGenClient {
	#storage: GroupInfoStorage & KeyGenInfoStorage;
	#logger: Logger;

	constructor(storage: GroupInfoStorage & KeyGenInfoStorage, logger: Logger) {
		this.#storage = storage;
		this.#logger = logger;
	}

	hasParticipant(groupId: GroupId, participant: Address): boolean {
		return this.#storage.hasParticipant(groupId, participant);
	}

	participants(groupId: GroupId): readonly Address[] {
		return this.#storage.participants(groupId);
	}

	threshold(groupId: GroupId): number {
		return this.#storage.threshold(groupId);
	}

	knownGroups(): GroupId[] {
		return this.#storage.knownGroups();
	}

	unregisterGroup(groupId: GroupId): void {
		this.#storage.unregisterGroup(groupId);
	}

	groupPublicKey(groupId: GroupId): FrostPoint | undefined {
		return this.#storage.publicKey(groupId);
	}

	missingCommitments(groupId: GroupId): Address[] {
		return this.#storage.missingCommitments(groupId);
	}

	missingSecretShares(groupId: GroupId, me: Address): Address[] {
		return this.#storage.missingSecretShares(groupId, me);
	}

	setupGroup(
		participants: readonly Address[],
		threshold: number,
		context: Hex,
	): {
		groupId: GroupId;
		participantsRoot: Hex;
		// TODO: allow to observe
	} {
		const participantsRoot = calculateParticipantsRoot(participants);
		const count = participants.length;
		const groupId = calcGroupId(participantsRoot, count, threshold, context);
		// TODO: [observe mode] calculate participant id elsewhere
		this.#storage.registerGroup(groupId, participants, threshold);
		return {
			groupId,
			participantsRoot,
		};
	}

	setupKeyGen(
		groupId: GroupId,
		me: Address,
		participants: Address[],
		threshold: number,
	): {
		encryptionPublicKey: FrostPoint;
		commitments: FrostPoint[];
		pok: ProofOfKnowledge;
		poap: ProofOfAttestationParticipation;
	} {
		const encryption = createEncryptionKey();
		const coefficients = createCoefficients(threshold);
		this.#storage.registerKeyGen(groupId, me, encryption.secretKey, coefficients);
		const pok = createProofOfKnowledge(deriveParticipantId(me), coefficients);
		const commitments = createCommitments(coefficients);
		const poap = generateParticipantProof(participants, me);
		return {
			pok,
			poap,
			encryptionPublicKey: encryption.publicKey,
			commitments,
		};
	}

	handleKeygenCommitment(
		groupId: GroupId,
		sender: Address,
		peerEncryptionPublicKey: FrostPoint,
		peerCommitments: readonly FrostPoint[],
		pok: ProofOfKnowledge,
	): boolean {
		if (!verifyCommitments(deriveParticipantId(sender), peerCommitments, pok)) return false;
		this.#storage.registerCommitments(groupId, sender, peerEncryptionPublicKey, peerCommitments);
		return true;
	}

	// Round 2.1
	createSecretShares(
		groupId: GroupId,
		me: Address,
	): {
		verificationShare: FrostPoint;
		shares: bigint[];
	} {
		const commitments = toParticipantIdMap(this.#storage.commitmentsMap(groupId));
		const groupPublicKey = createVerificationShare(commitments, 0n);
		this.#storage.registerGroupKey(groupId, groupPublicKey);
		// TODO: [observe mode] allow to register group public key
		// Will be published as y
		const verificationShare = createVerificationShare(commitments, deriveParticipantId(me));
		this.#storage.registerVerificationShare(groupId, me, verificationShare);

		const encryptionSecretKey = this.#storage.encryptionSecretKey(groupId, me);
		const coefficients = this.#storage.coefficients(groupId, me);
		const participants = this.#storage.participants(groupId);
		const shares: bigint[] = [];
		for (const peer of participants) {
			if (peer === me) continue;
			const peerId = deriveParticipantId(peer);
			// TODO: [observe mode] remove - peerCommitments are not used here anymore (previously it was utilized for encryption)
			const peerCommitments = commitments.get(peerId);
			if (peerCommitments === undefined) throw new Error(`Commitments for ${groupId}:${peer} are not available!`);
			const peerEncryptionPublicKey = this.#storage.encryptionPublicKey(groupId, peer);
			const peerShare = evalPoly(coefficients, peerId);
			const encryptedShare = ecdh(peerShare, encryptionSecretKey, peerEncryptionPublicKey);
			shares.push(encryptedShare);
		}
		if (shares.length !== participants.length - 1) {
			throw new Error("Unexpect f length");
		}
		return {
			verificationShare,
			shares,
		};
	}

	// Complaint flow reveal
	createSecretShare(groupId: GroupId, me: Address, peer: Address): bigint {
		const coefficients = this.#storage.coefficients(groupId, me);
		return evalPoly(coefficients, deriveParticipantId(peer));
	}

	// Complaint flow verify revealed
	verifySecretShare(groupId: GroupId, me: Address, peer: Address, secretShare: bigint): boolean {
		const commitment = this.#storage.commitments(groupId, peer);
		if (commitment === undefined) throw new Error(`Commitments for ${groupId}:${peer} are not available!`);
		const partialVerificationShare = evalCommitment(commitment, deriveParticipantId(me));
		return verifyKey(partialVerificationShare, secretShare);
	}

	protected finalizeSharesIfPossible(groupId: GroupId, me: Address): "pending_shares" | "shares_completed" {
		if (this.#storage.checkIfSecretSharesComplete(groupId, me)) {
			const verificationShare = this.#storage.verificationShare(groupId, me);
			const secretShares = toParticipantIdMap(this.#storage.secretSharesMap(groupId, me));
			const signingShare = createSigningShare(secretShares);
			if (!verifyKey(verificationShare, signingShare)) {
				throw new Error("Invalid signing share reconstructed!");
			}
			this.#storage.registerSigningShare(groupId, me, signingShare);
			this.#storage.clearKeyGen(groupId);
			return "shares_completed";
		}
		return "pending_shares";
	}

	protected registerSecretShare(
		groupId: GroupId,
		me: Address,
		peer: Address,
		secretShare: bigint,
	): "pending_shares" | "shares_completed" {
		this.#storage.registerSecretShare(groupId, me, peer, secretShare);
		return this.finalizeSharesIfPossible(groupId, me);
	}

	async registerPlainKeyGenSecret(
		groupId: GroupId,
		me: Address,
		peer: Address,
		secretShare: bigint,
	): Promise<"invalid_share" | "pending_shares" | "shares_completed"> {
		if (!this.verifySecretShare(groupId, me, peer, secretShare)) {
			return "invalid_share";
		}
		return this.registerSecretShare(groupId, me, peer, secretShare);
	}

	// `senderId` is the id of sending local participant in the participants set
	// `peerShares` are the calculated and encrypted shares (also defined as `f`)
	async handleKeygenSecrets(
		groupId: GroupId,
		me: Address,
		peer: Address,
		peerShares: readonly bigint[],
	): Promise<"invalid_share" | "pending_shares" | "shares_completed"> {
		const participants = this.#storage.participants(groupId);
		if (peerShares.length !== participants.length - 1) {
			// Invalid data was submitted, flag this so a complaint can be issued
			return "invalid_share";
		}
		const participantId = deriveParticipantId(me);
		if (peer === me) {
			this.#logger.debug("Register own shares");
			const coefficients = this.#storage.coefficients(groupId, me);
			return this.registerSecretShare(groupId, me, me, evalPoly(coefficients, participantId));
		}
		const shareIndex = participants.filter((p) => p !== peer).findIndex((p) => p === me);
		if (shareIndex < 0) throw new Error("Could not find self in participants");
		const encryptionSecretKey = this.#storage.encryptionSecretKey(groupId, me);
		const commitments = this.#storage.commitments(groupId, peer);
		const peerEncryptionPublicKey = this.#storage.encryptionPublicKey(groupId, peer);
		if (commitments === undefined) throw new Error(`Commitments for ${groupId}:${peer} are not available!`);
		const partialShare = ecdh(peerShares[shareIndex], encryptionSecretKey, peerEncryptionPublicKey);
		const partialVerificationShare = evalCommitment(commitments, participantId);
		if (!verifyKey(partialVerificationShare, partialShare)) {
			// Share is invalid, abort as this would result in an invalid signing share
			return "invalid_share";
		}
		return this.registerSecretShare(groupId, me, peer, partialShare);
	}
}
