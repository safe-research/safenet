import { type Address, encodePacked, type Hex, keccak256 } from "viem";
import { deriveParticipantId, sortedParticipantIds, toParticipantIdMap } from "../../frost/identifier.js";
import type { FrostPoint, GroupId, SignatureId } from "../../frost/types.js";
import { generateMerkleProofWithRoot } from "../merkle.js";
import type { GroupInfoStorage, SignatureRequestStorage } from "../storage/types.js";
import { groupChallenge, lagrangeCoefficient } from "./group.js";
import {
	bindingFactors,
	calculateGroupCommitment,
	createNonceTree,
	decodeSequence,
	groupCommitmentShares,
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

	handleNonceCommitmentsHash(groupId: GroupId, sender: Address, nonceCommitmentsHash: Hex, chunk: bigint) {
		const participant = this.#storage.participant(groupId);
		// Only link own nonce commitments
		if (sender !== participant) return;
		this.#storage.linkNonceTree(groupId, chunk, nonceCommitmentsHash);
	}

	createNonceCommitments(
		groupId: GroupId,
		signatureId: SignatureId,
		message: Hex,
		sequence: bigint,
		signers: readonly Address[],
	): {
		nonceCommitments: PublicNonceCommitments;
		nonceProof: Hex[];
	} {
		if (signers.length < this.#storage.threshold(groupId)) {
			throw new Error("Not enough signers to start signing process");
		}
		// Check that signers are a subset of participants
		const participantsSet = new Set(this.participants(groupId));
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
		const participant = this.#storage.participant(groupId);
		this.#storage.registerNonceCommitments(signatureId, participant, nonceCommitments);
		return {
			nonceCommitments,
			nonceProof,
		};
	}

	handleNonceCommitments(signatureId: SignatureId, peer: Address, nonceCommitments: PublicNonceCommitments): boolean {
		const groupId = this.#storage.signingGroup(signatureId);
		const signer = this.#storage.participant(groupId);
		// Skip own commits
		if (signer === peer) return false;
		this.#storage.registerNonceCommitments(signatureId, peer, nonceCommitments);

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
		const signers = this.signers(signatureId);
		const signer = this.#storage.participant(groupId);

		// Derive the FROST identifiers from the signer addresses.
		const signerIds = sortedParticipantIds(signers);
		const signerIndex = signerIds.indexOf(deriveParticipantId(signer));

		const groupPublicKey = this.#storage.publicKey(groupId);
		if (groupPublicKey === undefined) throw new Error(`Missing public key for group ${groupId}`);

		const signingShare = this.#storage.signingShare(groupId);
		if (signingShare === undefined) throw new Error(`Missing signing share for group ${groupId}`);

		const signerNonceCommitments = toParticipantIdMap(this.#storage.nonceCommitmentsMap(signatureId));
		const message = this.#storage.message(signatureId);

		// Calculate information over the complete signer group
		const bindingFactorList = bindingFactors(groupPublicKey, signerIds, signerNonceCommitments, message);
		const commitmentShares = groupCommitmentShares(bindingFactorList, signerNonceCommitments);
		const groupCommitment = calculateGroupCommitment(commitmentShares);
		const challenge = groupChallenge(groupCommitment, groupPublicKey, message);
		const signerParts = signerIds.map((signerId, index) => {
			const nonceCommitments = signerNonceCommitments.get(signerId);
			if (nonceCommitments === undefined) {
				throw new Error(`Missing nonce commitments for ${signerId}`);
			}
			const r = commitmentShares[index];
			const coeff = lagrangeCoefficient(signerIds, signerId);
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

		if (!verifySignatureShare(signatureShare, this.#storage.verificationShare(groupId), signerPart.cl, signerPart.r)) {
			// This should never happen as all inputs have been verified before
			throw new Error("Could not create valid signature share!");
		}

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

	signers(signatureId: SignatureId): Address[] {
		return this.#storage.signers(signatureId);
	}

	signingGroup(signatureId: SignatureId): GroupId {
		return this.#storage.signingGroup(signatureId);
	}

	participants(groupId: GroupId): Address[] {
		return this.#storage.participants(groupId).slice();
	}

	missingNonces(groupId: GroupId): Address[] {
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

	threshold(groupId: GroupId): number {
		return this.#storage.threshold(groupId);
	}

	participant(groupId: GroupId): Address {
		return this.#storage.participant(groupId);
	}
}
