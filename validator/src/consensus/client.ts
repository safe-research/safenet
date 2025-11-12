import type { Address, Hex } from "viem";
import {
	createSigningShare,
	createVerificationShare,
	evalCommitment,
	evalPoly,
	verifyKey,
} from "../frost/math.js";
import { ecdh } from "../frost/secret.js";
import type { FrostPoint, GroupId, ProofOfKnowledge } from "../frost/types.js";
import {
	createCoefficients,
	createCommitments,
	createProofOfKnowledge,
	verifyCommitments,
} from "../frost/vss.js";
import {
	calculateParticipantsRoot,
	generateParticipantProof,
} from "./merkle.js";
import type { FrostCoordinator, Participant } from "./types.js";

type KeygenInfo = {
	groupId: GroupId;
	participants: Participant[];
	coefficients: bigint[];
	participantIndex: bigint;
	commitments: Map<bigint, readonly FrostPoint[]>;
	secretShares: Map<bigint, bigint>;
	verificationShare?: FrostPoint;
	signingShare?: bigint;
};

const findParticipationIndex = (
	participants: Participant[],
	validatorAddress: Address,
): [bigint, number] => {
	const arrayIndex = participants.findIndex(
		(p) => p.address === validatorAddress,
	);
	if (arrayIndex < 0)
		throw Error("Cannot find participant in participants array!");
	const participantIndex = participants.at(arrayIndex)?.index;
	if (participantIndex === undefined)
		throw Error("Cannot determine FROST index of participant!");
	return [participantIndex, arrayIndex];
};

const checkInformationComplete = (
	participants: Participant[],
	information: Map<bigint, unknown>,
): boolean => {
	for (const participant of participants) {
		if (!information.has(participant.index)) {
			return false;
		}
	}
	return true;
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
export class FrostClient {
	#coordinator: FrostCoordinator;
	#validatorAddress: Address;
	// TODO: extract into storage object to iterate on secure storage
	#participantsInfo = new Map<Hex, Participant[]>();
	#keyGenInfo = new Map<GroupId, KeygenInfo>();

	constructor(validatorAddress: Address, coordinator: FrostCoordinator) {
		this.#validatorAddress = validatorAddress;
		this.#coordinator = coordinator;
	}

	validator(): Address {
		return this.#validatorAddress;
	}

	participationIndex(groupId: GroupId): bigint {
		return this.#keyGenInfo.get(groupId)?.participantIndex ?? -1n;
	}

	registerParticipants(participants: Participant[]) {
		const participantsRoot = calculateParticipantsRoot(participants);
		this.#participantsInfo.set(participantsRoot, participants);
	}

	abortKeygen(groupId: GroupId) {
		if (!this.#keyGenInfo.has(groupId)) return;
		this.#keyGenInfo.delete(groupId);
	}

	async handleKeygenInit(
		groupId: GroupId,
		participantsRoot: Hex,
		count: bigint,
		threshold: bigint,
	) {
		if (this.#keyGenInfo.has(groupId))
			throw Error(`Key generation for ${groupId} has already been initialized`);
		const participants = this.#participantsInfo.get(participantsRoot);
		if (participants === undefined)
			throw Error(`Unknown root hash ${participantsRoot} for participants`);
		if (participants.length !== Number(count))
			throw Error(
				`Unexpected participant count ${participantsRoot}! (Expected ${participants.length} got ${count})`,
			);
		const [participantIndex, arrayIndex] = findParticipationIndex(
			participants,
			this.#validatorAddress,
		);
		const coefficients = createCoefficients(threshold);
		const pok = createProofOfKnowledge(groupId, participantIndex, coefficients);
		const localCommitments = createCommitments(coefficients);
		const poap = generateParticipantProof(participants, arrayIndex);
		const commitments = new Map<bigint, FrostPoint[]>();
		commitments.set(participantIndex, localCommitments);
		const secretShares = new Map<bigint, bigint>();
		secretShares.set(
			participantIndex,
			evalPoly(coefficients, participantIndex),
		);
		this.#keyGenInfo.set(groupId, {
			groupId,
			participants,
			participantIndex,
			coefficients,
			commitments,
			secretShares,
		});
		await this.#coordinator.publishKeygenCommitments(
			groupId,
			participantIndex,
			localCommitments,
			pok,
			poap,
		);
	}

	async handleKeygenCommitment(
		groupId: GroupId,
		senderIndex: bigint,
		peerCommitments: readonly FrostPoint[],
		pok: ProofOfKnowledge,
	) {
		const info = this.#keyGenInfo.get(groupId);
		if (info === undefined) return;
		if (senderIndex === info.participantIndex) {
			console.info("Do not verify own commitments");
			return;
		}
		if (info.commitments.has(senderIndex)) {
			throw Error(`Commitment for ${groupId}:${senderIndex} already known!`);
		}
		verifyCommitments(groupId, senderIndex, peerCommitments, pok);
		info.commitments.set(senderIndex, peerCommitments);
		if (checkInformationComplete(info.participants, info.commitments)) {
			await this.prepareAndPublishKeygenSecretShares(info);
		}
	}

	// Round 2.1
	private async prepareAndPublishKeygenSecretShares(info: KeygenInfo) {
		// Will be published as y
		const verificationShare = createVerificationShare(
			info.commitments,
			info.participantIndex,
		);
		info.verificationShare = verificationShare;

		const shares: bigint[] = [];
		for (const participant of info.participants) {
			if (participant.index === info.participantIndex) continue;
			const peerCommitments = info.commitments.get(participant.index);
			if (peerCommitments === undefined)
				throw Error(
					`Commitments for ${info.groupId}:${participant.index} are not available!`,
				);
			const peerShare = evalPoly(info.coefficients, participant.index);
			const encryptedShare = ecdh(
				peerShare,
				info.coefficients[0],
				peerCommitments[0],
			);
			shares.push(encryptedShare);
		}
		if (shares.length !== info.participants.length - 1) {
			throw Error("Unexpect f length");
		}
		await this.#coordinator.publishKeygenSecretShares(
			info.groupId,
			info.participantIndex,
			verificationShare,
			shares,
		);
	}

	// `senderIndex` is the index of sending local participant in the participants set
	// `peerShares` are the calculated and encrypted shares (also defined as `f`)
	async handleKeygenSecrets(
		groupId: GroupId,
		senderIndex: bigint,
		peerShares: readonly bigint[],
	) {
		const info = this.#keyGenInfo.get(groupId);
		if (info === undefined) return;
		if (peerShares.length !== info.participants.length - 1) {
			throw Error("Unexpect f length");
		}
		if (senderIndex === info.participantIndex) {
			console.info("Do not handle own share");
			return;
		}
		const [participantIndex] = findParticipationIndex(
			info.participants,
			this.#validatorAddress,
		);
		const commitment = info.commitments.get(senderIndex);
		if (commitment === undefined)
			throw Error(
				`Commitments for ${groupId}:${senderIndex} are not available!`,
			);
		// TODO: check if we should use a reasonable limit for the index (current uint256)
		const shareIndex =
			participantIndex < senderIndex ? participantIndex : participantIndex - 1n;
		// Note: Number(shareIndex) is theoretically an unsafe cast
		const partialShare = ecdh(
			peerShares[Number(shareIndex) - 1],
			info.coefficients[0],
			commitment[0],
		);
		const partialVerificationShare = evalCommitment(
			commitment,
			participantIndex,
		);
		verifyKey(partialVerificationShare, partialShare);
		info.secretShares.set(senderIndex, partialShare);

		if (checkInformationComplete(info.participants, info.secretShares)) {
			const verificationShare = info.verificationShare;
			if (verificationShare === undefined)
				throw Error("No verification share available!");
			const signingShare = createSigningShare(info.secretShares);
			verifyKey(verificationShare, signingShare);
			info.signingShare = signingShare;
			// TODO: cleanup stored information
			console.info(`Final signing key for ${info.participantIndex} calculated`);
		}
	}
}
