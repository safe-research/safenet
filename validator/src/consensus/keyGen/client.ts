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
	ProofOfKnowledge,
} from "../../frost/types.js";
import {
	createCoefficients,
	createCommitments,
	createProofOfKnowledge,
	verifyCommitments,
} from "../../frost/vss.js";
import { generateParticipantProof } from "../merkle.js";
import type {
	GroupInfoStorage,
	KeyGenCoordinator,
	KeyGenInfoStorage,
	Participant,
} from "../types.js";

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

export type KeyGenCallbacks = {
	onGroupSetup?: (groupId: GroupId, participantId: ParticipantId) => void;
	onDebug?: (log: string) => void;
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
	#coordinator: KeyGenCoordinator;
	#storage: GroupInfoStorage & KeyGenInfoStorage;
	#callbacks: KeyGenCallbacks;

	constructor(
		storage: GroupInfoStorage & KeyGenInfoStorage,
		coordinator: KeyGenCoordinator,
		callbacks: KeyGenCallbacks = {},
	) {
		this.#storage = storage;
		this.#coordinator = coordinator;
		this.#callbacks = callbacks;
	}

	participationId(groupId: GroupId): bigint {
		return this.#storage.participantId(groupId);
	}

	knownGroups(): Hex[] {
		return this.#storage.knownGroups();
	}

	groupPublicKey(groupId: GroupId): FrostPoint | undefined {
		return this.#storage.publicKey(groupId);
	}

	registerParticipants(participants: Participant[]) {
		this.#storage.registerParticipants(participants);
	}

	abortKeygen(groupId: GroupId) {
		this.#storage.unregisterGroup(groupId);
	}

	async handleKeygenInit(
		groupId: GroupId,
		participantsRoot: Hex,
		count: bigint,
		threshold: bigint,
	) {
		const participants = this.#storage.loadParticipants(participantsRoot);
		if (participants.length !== Number(count))
			throw Error(
				`Unexpected participant count ${participantsRoot}! (Expected ${participants.length} got ${count})`,
			);
		const participantId = this.#storage.registerGroup(groupId, participants);
		const coefficients = createCoefficients(threshold);
		this.#storage.registerKeyGen(groupId, coefficients);
		const pok = createProofOfKnowledge(participantId, coefficients);
		const localCommitments = createCommitments(coefficients);
		const poap = generateParticipantProof(participants, participantId);
		this.#storage.registerCommitments(groupId, participantId, localCommitments);
		this.#storage.registerSecretShare(
			groupId,
			participantId,
			evalPoly(coefficients, participantId),
		);
		await this.#coordinator.publishKeygenCommitments(
			groupId,
			participantId,
			localCommitments,
			pok,
			poap,
		);
	}

	async handleKeygenCommitment(
		groupId: GroupId,
		senderId: ParticipantId,
		peerCommitments: readonly FrostPoint[],
		pok: ProofOfKnowledge,
	) {
		const participantId = this.#storage.participantId(groupId);
		if (senderId === participantId) {
			this.#callbacks.onDebug?.("Do not verify own commitments");
			return;
		}
		verifyCommitments(senderId, peerCommitments, pok);
		this.#storage.registerCommitments(groupId, senderId, peerCommitments);
		if (this.#storage.checkIfCommitmentsComplete(groupId)) {
			await this.prepareAndPublishKeygenSecretShares(groupId);
		}
	}

	// Round 2.1
	private async prepareAndPublishKeygenSecretShares(groupId: GroupId) {
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
		await this.#coordinator.publishKeygenSecretShares(
			groupId,
			verificationShare,
			shares,
		);
	}

	// `senderId` is the index of sending local participant in the participants set
	// `peerShares` are the calculated and encrypted shares (also defined as `f`)
	async handleKeygenSecrets(
		groupId: GroupId,
		senderId: ParticipantId,
		peerShares: readonly bigint[],
	) {
		const participants = this.#storage.participants(groupId);
		if (peerShares.length !== participants.length - 1) {
			throw Error("Unexpect f length");
		}
		const participantId = this.#storage.participantId(groupId);
		if (senderId === participantId) {
			this.#callbacks.onDebug?.("Do not handle own share");
			return;
		}
		const commitment = this.#storage.commitments(groupId, senderId);
		if (commitment === undefined)
			throw Error(`Commitments for ${groupId}:${senderId} are not available!`);
		// TODO: check if we should use a reasonable limit for the index (current uint256)
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
			const participantId = this.#storage.participantId(groupId);
			this.#callbacks.onGroupSetup?.(groupId, participantId);
		}
	}
}
