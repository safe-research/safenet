import type { Hex } from "viem";
import {
	createSigningShare,
	createVerificationShare,
	evalCommitment,
	evalPoly,
	verifyKey,
} from "../../frost/math.js";
import { ecdh } from "../../frost/secret.js";
import type {
	FrostPoint,
	GroupId,
	ParticipantId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
} from "../../frost/types.js";
import {
	createCoefficients,
	createCommitments,
	createProofOfKnowledge,
	verifyCommitments,
} from "../../frost/vss.js";
import type { Logger } from "../../utils/logging.js";
import { generateParticipantProof } from "../merkle.js";
import type {
	GroupInfoStorage,
	KeyGenInfoStorage,
	Participant,
} from "../storage/types.js";
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
 *   a. recive commitments from other participants
 * 4. publish secret shares
 *   a. receive secret shares
 */
export class KeyGenClient {
	#storage: GroupInfoStorage & KeyGenInfoStorage;
	#logger?: Logger;

	constructor(storage: GroupInfoStorage & KeyGenInfoStorage, logger?: Logger) {
		this.#storage = storage;
		this.#logger = logger;
	}

	participantId(groupId: GroupId): bigint {
		return this.#storage.participantId(groupId);
	}

	knownGroups(): GroupId[] {
		return this.#storage.knownGroups();
	}

	groupPublicKey(groupId: GroupId): FrostPoint | undefined {
		return this.#storage.publicKey(groupId);
	}

	registerParticipants(participants: Participant[]): Hex {
		return this.#storage.registerParticipants(participants);
	}

	abortKeygen(groupId: GroupId) {
		this.#storage.unregisterGroup(groupId);
	}

	setupGroup(
		participantsRoot: Hex,
		count: bigint,
		threshold: bigint,
		context: Hex,
	): {
		groupId: GroupId;
		participantId: bigint;
		commitments: FrostPoint[];
		pok: ProofOfKnowledge;
		poap: ProofOfAttestationParticipation;
	} {
		const groupId = calcGroupId(participantsRoot, count, threshold, context);
		const participants = this.#storage.loadParticipants(participantsRoot);
		if (participants.length !== Number(count))
			throw Error(
				`Unexpected participant count ${participantsRoot}! (Expected ${participants.length} got ${count})`,
			);
		const participantId = this.#storage.registerGroup(
			groupId,
			participants,
			threshold,
		);
		const coefficients = createCoefficients(threshold);
		this.#storage.registerKeyGen(groupId, coefficients);
		const pok = createProofOfKnowledge(participantId, coefficients);
		const commitments = createCommitments(coefficients);
		const poap = generateParticipantProof(participants, participantId);
		this.#storage.registerCommitments(groupId, participantId, commitments);
		this.#storage.registerSecretShare(
			groupId,
			participantId,
			evalPoly(coefficients, participantId),
		);
		return {
			groupId,
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
		const participantId = this.#storage.participantId(groupId);
		if (senderId === participantId) {
			this.#logger?.debug("Do not verify own commitments");
			return false;
		}
		verifyCommitments(senderId, peerCommitments, pok);
		this.#storage.registerCommitments(groupId, senderId, peerCommitments);
		return this.#storage.checkIfCommitmentsComplete(groupId);
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
		const verificationShare = createVerificationShare(
			commitments,
			participantId,
		);
		this.#storage.registerVerification(
			groupId,
			groupPublicKey,
			verificationShare,
		);

		const coefficients = this.#storage.coefficients(groupId);
		const participants = this.#storage.participants(groupId);
		const shares: bigint[] = [];
		for (const participant of participants) {
			if (participant.id === participantId) continue;
			const peerCommitments = commitments.get(participant.id);
			if (peerCommitments === undefined)
				throw Error(
					`Commitments for ${groupId}:${participant.id} are not available!`,
				);
			const peerShare = evalPoly(coefficients, participant.id);
			const encryptedShare = ecdh(
				peerShare,
				coefficients[0],
				peerCommitments[0],
			);
			shares.push(encryptedShare);
		}
		if (shares.length !== participants.length - 1) {
			throw Error("Unexpect f length");
		}
		return {
			verificationShare,
			shares,
		};
	}

	// `senderId` is the id of sending local participant in the participants set
	// `peerShares` are the calculated and encrypted shares (also defined as `f`)
	async handleKeygenSecrets(
		groupId: GroupId,
		senderId: ParticipantId,
		peerShares: readonly bigint[],
	): Promise<boolean> {
		const participants = this.#storage.participants(groupId);
		if (peerShares.length !== participants.length - 1) {
			throw Error("Unexpect f length");
		}
		const participantId = this.#storage.participantId(groupId);
		if (senderId === participantId) {
			this.#logger?.debug("Do not handle own share");
			return false;
		}
		const commitment = this.#storage.commitments(groupId, senderId);
		if (commitment === undefined)
			throw Error(`Commitments for ${groupId}:${senderId} are not available!`);
		// TODO: check if we should use a reasonable limit for the id (current uint256)
		const shareIndex =
			participantId < senderId ? participantId : participantId - 1n;
		// Note: Number(shareIndex) is theoretically an unsafe cast
		const key = this.#storage.encryptionKey(groupId);
		const partialShare = ecdh(
			peerShares[Number(shareIndex) - 1],
			key,
			commitment[0],
		);
		const partialVerificationShare = evalCommitment(commitment, participantId);
		verifyKey(partialVerificationShare, partialShare);
		this.#storage.registerSecretShare(groupId, senderId, partialShare);

		if (this.#storage.checkIfSecretSharesComplete(groupId)) {
			const verificationShare = this.#storage.verificationShare(groupId);
			const secretShares = this.#storage.secretSharesMap(groupId);
			const signingShare = createSigningShare(secretShares);
			verifyKey(verificationShare, signingShare);
			this.#storage.registerSigningShare(groupId, signingShare);
			this.#storage.clearKeyGen(groupId);
			return true;
		}
		return false;
	}
}
