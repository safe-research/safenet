import { encodePacked, type Hex, keccak256 } from "viem";
import type { FrostPoint, GroupId, ParticipantId, SignatureId } from "../../frost/types.js";
import { generateMerkleProofWithRoot } from "../merkle.js";
import type { GroupInfoStorage, SignatureRequestStorage } from "../storage/types.js";
import { groupChallenge, lagrangeCoefficient } from "./group.js";
import {
	bindingFactors,
	calculateGroupCommitment,
	createNonceTree,
	decodeSequence,
	groupCommitementShares,
	nonceCommitmentsWithProof,
	type PublicNonceCommitments,
} from "./nonces.js";
import { createSignatureShare, lagrangeChallenge } from "./shares.js";
import { verifySignatureShare } from "./verify.js";

export class SigningClient {
	#storage: GroupInfoStorage & SignatureRequestStorage;

	constructor(storage: GroupInfoStorage & SignatureRequestStorage) {
		this.#storage = storage;
	}

	generateNonceTree(groupId: GroupId): Hex {
		const signingShare = this.#storage.signingShare(groupId);
		if (signingShare === undefined) throw new Error(`No info for ${groupId}`);
		const nonceTree = createNonceTree(signingShare);
		const nonceTreeRoot = this.#storage.registerNonceTree(groupId, nonceTree);
		return nonceTreeRoot;
	}

	handleNonceCommitmentsHash(groupId: GroupId, senderId: ParticipantId, nonceCommitmentsHash: Hex, chunk: bigint) {
		const participantId = this.#storage.participantId(groupId);
		// Only link own nonce commitments
		if (participantId !== senderId) return;
		this.#storage.linkNonceTree(groupId, chunk, nonceCommitmentsHash);
	}

	createNonceCommitments(
		groupId: GroupId,
		signatureId: SignatureId,
		message: Hex,
		sequence: bigint,
		signers: readonly ParticipantId[],
	): {
		nonceCommitments: PublicNonceCommitments;
		nonceProof: Hex[];
	} {
		if (signers.length < this.#storage.threshold(groupId)) {
			throw new Error("Not enough signers to start signing process");
		}
		const participantsSet = new Set(this.#storage.participants(groupId).map((p) => p.id));
		for (const signer of signers) {
			if (!participantsSet.has(signer)) {
				throw new Error(`Invalid signer id provided: ${signer}`);
			}
		}
		this.#storage.registerSignatureRequest(signatureId, groupId, message, signers, sequence);
		// Set own nonce commitments
		const { chunk, offset } = decodeSequence(sequence);
		const nonceTree = this.#storage.nonceTree(groupId, chunk);
		const { nonceCommitments, nonceProof } = nonceCommitmentsWithProof(nonceTree, offset);
		const participantId = this.#storage.participantId(groupId);
		this.#storage.registerNonceCommitments(signatureId, participantId, nonceCommitments);
		return {
			nonceCommitments,
			nonceProof,
		};
	}

	handleNonceCommitments(
		signatureId: SignatureId,
		peerId: ParticipantId,
		nonceCommitments: PublicNonceCommitments,
	): boolean {
		const groupId = this.#storage.signingGroup(signatureId);
		const signerId = this.#storage.participantId(groupId);
		// Skip own commits
		if (signerId === peerId) return false;
		this.#storage.registerNonceCommitments(signatureId, peerId, nonceCommitments);

		return this.#storage.checkIfNoncesComplete(signatureId);
	}

	createSignatureShare(signatureId: SignatureId): {
		signersRoot: Hex;
		signersProof: Hex[];
		groupCommitment: FrostPoint;
		commitmentShare: FrostPoint;
		signatureShare: bigint;
		lagrangeCoefficient: bigint;
	} {
		const groupId = this.#storage.signingGroup(signatureId);
		const signers = this.#storage.signers(signatureId);
		const signerId = this.#storage.participantId(groupId);

		const groupPublicKey = this.#storage.publicKey(groupId);
		if (groupPublicKey === undefined) throw new Error(`Missing public key for group ${groupId}`);

		const signingShare = this.#storage.signingShare(groupId);
		if (signingShare === undefined) throw new Error(`Missing signing share for group ${groupId}`);

		const signerIndex = signers.indexOf(signerId);
		const signerNonceCommitments = this.#storage.nonceCommitmentsMap(signatureId);
		const message = this.#storage.message(signatureId);

		// Calculate information over the complete signer group
		const bindingFactorList = bindingFactors(groupPublicKey, signers, signerNonceCommitments, message);
		const groupCommitmentShares = groupCommitementShares(bindingFactorList, signerNonceCommitments);
		const groupCommitment = calculateGroupCommitment(groupCommitmentShares);
		const challenge = groupChallenge(groupCommitment, groupPublicKey, message);
		const signerParts = signers.map((signerId, index) => {
			const nonceCommitments = signerNonceCommitments.get(signerId);
			if (nonceCommitments === undefined) {
				throw new Error(`Missing nonce commitments for ${signerId}`);
			}
			const r = groupCommitmentShares[index];
			const coeff = lagrangeCoefficient(signers, signerId);
			const cl = lagrangeChallenge(coeff, challenge);
			const node = keccak256(
				encodePacked(
					["uint256", "uint256", "uint256", "uint256", "uint256", "uint256"],
					[signerId, r.x, r.y, coeff, groupCommitment.x, groupCommitment.y],
				),
			);
			return {
				signerId,
				r,
				l: coeff,
				cl,
				node,
			};
		});

		const sequence = this.#storage.sequence(signatureId);
		const { chunk, offset } = decodeSequence(sequence);
		const nonceTree = this.#storage.nonceTree(groupId, chunk);
		// Calculate information specific to this signer
		const nonceCommitments = nonceTree.commitments[Number(offset)];
		if (nonceCommitments.bindingNonce === 0n && nonceCommitments.hidingNonce === 0n) {
			throw new Error(`Nonces for sequence ${sequence} have been already burned`);
		}
		const signerPart = signerParts[signerIndex];
		const signatureShare = createSignatureShare(
			signingShare,
			nonceCommitments,
			bindingFactorList[signerIndex].bindingFactor,
			signerPart.cl,
		);
		const { proof: signersProof, root: signersRoot } = generateMerkleProofWithRoot(
			signerParts.map((p) => p.node),
			signerIndex,
		);

		verifySignatureShare(signatureShare, this.#storage.verificationShare(groupId), signerPart.cl, signerPart.r);

		this.#storage.burnNonce(groupId, chunk, offset);

		return {
			signersRoot,
			signersProof,
			groupCommitment,
			commitmentShare: signerPart.r,
			signatureShare,
			lagrangeCoefficient: signerPart.l,
		};
	}

	signers(groupId: GroupId): ParticipantId[] {
		return this.#storage.signers(groupId);
	}

	missingNonces(groupId: GroupId): ParticipantId[] {
		return this.#storage.missingNonces(groupId);
	}

	availableNoncesCount(groupId: GroupId, chunk: bigint): bigint {
		try {
			const nonceTree = this.#storage.nonceTree(groupId, chunk);
			return BigInt(nonceTree.leaves.length);
		} catch {
			return 0n;
		}
	}

	signingGroup(signatureId: SignatureId): GroupId {
		return this.#storage.signingGroup(signatureId);
	}

	message(signatureId: SignatureId): Hex {
		return this.#storage.message(signatureId);
	}

	threshold(signatureId: SignatureId): bigint {
		const groupId = this.#storage.signingGroup(signatureId);
		return this.#storage.threshold(groupId);
	}

	requiredShareCount(signatureId: SignatureId): bigint {
		return BigInt(this.#storage.signers(signatureId).length);
	}

	participantId(signatureId: SignatureId): bigint {
		const groupId = this.#storage.signingGroup(signatureId);
		return this.#storage.participantId(groupId);
	}
}
