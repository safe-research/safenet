import type { Hex } from "viem";
import { createSigningShare, createVerificationShare, evalCommitment, evalPoly, verifyKey } from "../../frost/math.js";
import { ecdh } from "../../frost/secret.js";
import type {
	FrostPoint,
	GroupId,
	ParticipantId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
} from "../../frost/types.js";
import { createCoefficients, createCommitments, createProofOfKnowledge, verifyCommitments } from "../../frost/vss.js";
import type { Logger } from "../../utils/logging.js";
import { calculateParticipantsRoot, generateParticipantProof } from "../merkle.js";
import type { GroupInfoStorage, KeyGenInfoStorage, Participant } from "../storage/types.js";
import { calcGroupId } from "./utils.js";

export type KeygenInfo = {
	groupId: GroupId;
	participants: Participant[];
	coefficients: bigint[];
	participantId: bigint;
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

	participantId(groupId: GroupId): bigint {
		return this.#storage.participantId(groupId);
	}

	participants(groupId: GroupId): readonly Participant[] {
		return this.#storage.participants(groupId);
	}

	threshold(groupId: GroupId): number {
		return this.#storage.threshold(groupId);
	}

	knownGroups(): GroupId[] {
		return this.#storage.knownGroups();
	}

	groupPublicKey(groupId: GroupId): FrostPoint | undefined {
		return this.#storage.publicKey(groupId);
	}

	missingCommitments(groupId: GroupId): ParticipantId[] {
		return this.#storage.missingCommitments(groupId);
	}

	missingSecretShares(groupId: GroupId): ParticipantId[] {
		return this.#storage.missingSecretShares(groupId);
	}

	setupGroup(
		participants: readonly Participant[],
		threshold: number,
		context: Hex,
	): {
		groupId: GroupId;
		participantsRoot: Hex;
		participantId: bigint;
		commitments: FrostPoint[];
		pok: ProofOfKnowledge;
		poap: ProofOfAttestationParticipation;
	} {
		const participantsRoot = calculateParticipantsRoot(participants);
		const count = participants.length;
		const groupId = calcGroupId(participantsRoot, count, threshold, context);
		const participantId = this.#storage.registerGroup(groupId, participants, threshold);
		const coefficients = createCoefficients(threshold);
		this.#storage.registerKeyGen(groupId, coefficients);
		const pok = createProofOfKnowledge(participantId, coefficients);
		const commitments = createCommitments(coefficients);
		const poap = generateParticipantProof(participants, participantId);
		return {
			groupId,
			participantsRoot,
			participantId,
			pok,
			poap,
			commitments,
		};
	}

	handleKeygenCommitment(
		groupId: GroupId,
		senderId: ParticipantId,
		peerCommitments: readonly FrostPoint[],
		pok: ProofOfKnowledge,
	): boolean {
		if (!verifyCommitments(senderId, peerCommitments, pok)) return false;
		this.#storage.registerCommitments(groupId, senderId, peerCommitments);
		return true;
	}

	// Round 2.1
	createSecretShares(groupId: GroupId): {
		verificationShare: FrostPoint;
		shares: bigint[];
	} {
		const commitments = this.#storage.commitmentsMap(groupId);
		const groupPublicKey = createVerificationShare(commitments, 0n);
		// Will be published as y
		const participantId = this.#storage.participantId(groupId);
		const verificationShare = createVerificationShare(commitments, participantId);
		this.#storage.registerVerification(groupId, groupPublicKey, verificationShare);

		const coefficients = this.#storage.coefficients(groupId);
		const participants = this.#storage.participants(groupId);
		const shares: bigint[] = [];
		for (const participant of participants) {
			if (participant.id === participantId) continue;
			const peerCommitments = commitments.get(participant.id);
			if (peerCommitments === undefined)
				throw new Error(`Commitments for ${groupId}:${participant.id} are not available!`);
			const peerShare = evalPoly(coefficients, participant.id);
			const encryptedShare = ecdh(peerShare, coefficients[0], peerCommitments[0]);
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
	createSecretShare(groupId: GroupId, peerId: ParticipantId): bigint {
		const coefficients = this.#storage.coefficients(groupId);
		return evalPoly(coefficients, peerId);
	}

	// Complaint flow verify revealed
	verifySecretShare(groupId: GroupId, senderId: ParticipantId, targetId: bigint, secretShare: bigint): boolean {
		const commitment = this.#storage.commitments(groupId, senderId);
		if (commitment === undefined) throw new Error(`Commitments for ${groupId}:${senderId} are not available!`);
		const partialVerificationShare = evalCommitment(commitment, targetId);
		return verifyKey(partialVerificationShare, secretShare);
	}

	protected finalizeSharesIfPossible(groupId: GroupId): "pending_shares" | "shares_completed" {
		if (this.#storage.checkIfSecretSharesComplete(groupId)) {
			const verificationShare = this.#storage.verificationShare(groupId);
			const secretShares = this.#storage.secretSharesMap(groupId);
			const signingShare = createSigningShare(secretShares);
			if (!verifyKey(verificationShare, signingShare)) {
				throw new Error("Invalid signing share reconstructed!");
			}
			this.#storage.registerSigningShare(groupId, signingShare);
			this.#storage.clearKeyGen(groupId);
			return "shares_completed";
		}
		return "pending_shares";
	}

	protected registerSecretShare(
		groupId: GroupId,
		senderId: ParticipantId,
		secretShare: bigint,
	): "pending_shares" | "shares_completed" {
		this.#storage.registerSecretShare(groupId, senderId, secretShare);
		return this.finalizeSharesIfPossible(groupId);
	}

	async registerPlainKeyGenSecret(
		groupId: GroupId,
		senderId: ParticipantId,
		secretShare: bigint,
	): Promise<"invalid_share" | "pending_shares" | "shares_completed"> {
		const participantId = this.#storage.participantId(groupId);
		if (!this.verifySecretShare(groupId, senderId, participantId, secretShare)) {
			return "invalid_share";
		}
		return this.registerSecretShare(groupId, senderId, secretShare);
	}

	// `senderId` is the id of sending local participant in the participants set
	// `peerShares` are the calculated and encrypted shares (also defined as `f`)
	async handleKeygenSecrets(
		groupId: GroupId,
		senderId: ParticipantId,
		peerShares: readonly bigint[],
	): Promise<"invalid_share" | "pending_shares" | "shares_completed"> {
		const participants = this.#storage.participants(groupId);
		if (peerShares.length !== participants.length - 1) {
			// Invalid data was submitted, flag this so a complaint can be issued
			return "invalid_share";
		}
		const participantId = this.#storage.participantId(groupId);
		if (senderId === participantId) {
			this.#logger.debug("Register own shares");
			const coefficients = this.#storage.coefficients(groupId);
			return this.registerSecretShare(groupId, participantId, evalPoly(coefficients, participantId));
		}
		const shareIndex = participants.filter(({ id }) => id !== senderId).findIndex(({ id }) => id === participantId);
		if (shareIndex < 0) throw new Error("Could not find self in participants");
		const key = this.#storage.encryptionKey(groupId);
		const commitments = this.#storage.commitments(groupId, senderId);
		if (commitments === undefined) throw new Error(`Commitments for ${groupId}:${senderId} are not available!`);
		const partialShare = ecdh(peerShares[shareIndex], key, commitments[0]);
		const partialVerificationShare = evalCommitment(commitments, participantId);
		if (!verifyKey(partialVerificationShare, partialShare)) {
			// Share is invalid, abort as this would result in an invalid signing share
			return "invalid_share";
		}
		return this.registerSecretShare(groupId, senderId, partialShare);
	}
}
