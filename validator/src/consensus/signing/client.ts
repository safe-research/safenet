import { encodePacked, type Hex, keccak256 } from "viem";
import type { GroupId, ParticipantId, SignatureId } from "../../frost/types.js";
import { generateMerkleProofWithRoot } from "../merkle.js";
import type {
	GroupInfoStorage,
	SignatureRequestStorage,
	SigningCoordinator,
} from "../types.js";
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

export type SigningCallbacks = {
	onRequestSigned?: (
		signatureId: SignatureId,
		signerId: ParticipantId,
		message: Hex,
	) => void;
	onDebug?: (log: string) => void;
};

export class SigningClient {
	#storage: GroupInfoStorage & SignatureRequestStorage;
	#coordinator: SigningCoordinator;
	#callbacks: SigningCallbacks;

	constructor(
		storage: GroupInfoStorage & SignatureRequestStorage,
		coordinator: SigningCoordinator,
		callbacks: SigningCallbacks = {},
	) {
		this.#storage = storage;
		this.#coordinator = coordinator;
		this.#callbacks = callbacks;
	}

	async commitNonces(groupId: GroupId): Promise<Hex> {
		const signingShare = this.#storage.signingShare(groupId);
		if (signingShare === undefined) throw Error(`No info for ${groupId}`);
		const nonceTree = createNonceTree(signingShare);
		const nonceTreeHash = this.#storage.registerNonceTree(nonceTree);
		await this.#coordinator.publishNonceCommitmentsHash(groupId, nonceTreeHash);
		return nonceTreeHash;
	}

	async handleNonceCommitmentsHash(
		groupId: GroupId,
		senderId: ParticipantId,
		nonceCommitmentsHash: Hex,
		chunk: bigint,
	) {
		const participantId = this.#storage.participantId(groupId);
		// Only link own nonce commitments
		if (participantId !== senderId) return;
		this.#storage.linkNonceTree(groupId, chunk, nonceCommitmentsHash);
	}

	async handleSignatureRequest(
		groupId: GroupId,
		signatureId: SignatureId,
		message: Hex,
		sequence: bigint,
	) {
		const participants = this.#storage.participants(groupId);
		// TODO: add check for unhonest signers
		const signers = participants.map((p) => p.id).sort();
		this.#storage.registerSignatureRequest(
			signatureId,
			groupId,
			message,
			signers,
			sequence,
		);
		// Set own nonce commitments
		const { chunk, offset } = decodeSequence(sequence);
		const nonceTree = this.#storage.nonceTree(groupId, chunk);
		const { nonceCommitments, nonceProof } = nonceCommitmentsWithProof(
			nonceTree,
			offset,
		);
		const participantId = this.#storage.participantId(groupId);
		this.#storage.registerNonceCommitments(
			signatureId,
			participantId,
			nonceCommitments,
		);
		await this.#coordinator.publishNonceCommitments(
			signatureId,
			nonceCommitments,
			nonceProof,
		);
	}

	async handleNonceCommitments(
		signatureId: SignatureId,
		peerId: ParticipantId,
		nonceCommitments: PublicNonceCommitments,
	): Promise<Hex | undefined> {
		const groupId = this.#storage.signingGroup(signatureId);
		const signerId = this.#storage.participantId(groupId);
		// Skip own commits
		if (signerId === peerId) return undefined;
		this.#storage.registerNonceCommitments(
			signatureId,
			peerId,
			nonceCommitments,
		);

		if (this.#storage.checkIfNoncesComplete(signatureId)) {
			return await this.submitSignature(signatureId);
		}
		return undefined;
	}

	private async submitSignature(signatureId: SignatureId): Promise<Hex> {
		const groupId = this.#storage.signingGroup(signatureId);
		const signers = this.#storage.signers(signatureId);
		const signerId = this.#storage.participantId(groupId);

		const groupPublicKey = this.#storage.publicKey(groupId);
		if (groupPublicKey === undefined)
			throw Error(`Missing public key for group ${groupId}`);

		const signingShare = this.#storage.signingShare(groupId);
		if (signingShare === undefined)
			throw Error(`Missing signing share for group ${groupId}`);

		const signerIndex = signers.indexOf(signerId);
		const signerNonceCommitments =
			this.#storage.nonceCommitmentsMap(signatureId);
		const message = this.#storage.message(signatureId);

		// Calculate information over the complete signer group
		const bindingFactorList = bindingFactors(
			groupPublicKey,
			signers,
			signerNonceCommitments,
			message,
		);
		const groupCommitmentShares = groupCommitementShares(
			bindingFactorList,
			signerNonceCommitments,
		);
		const groupCommitment = calculateGroupCommitment(groupCommitmentShares);
		const challenge = groupChallenge(groupCommitment, groupPublicKey, message);
		const signerParts = signers.map((signerId, index) => {
			const nonceCommitments = signerNonceCommitments.get(signerId);
			if (nonceCommitments === undefined)
				throw Error(`Missing nonce commitments for ${signerId}`);
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
		if (
			nonceCommitments.bindingNonce === 0n &&
			nonceCommitments.hidingNonce === 0n
		) {
			throw Error(`Nonces for sequence ${sequence} have been already burned`);
		}
		const signerPart = signerParts[signerIndex];
		const signatureShare = createSignatureShare(
			signingShare,
			nonceCommitments,
			bindingFactorList[signerIndex].bindingFactor,
			signerPart.cl,
		);
		const { proof: signingParticipantsProof, root: signingParticipantsHash } =
			generateMerkleProofWithRoot(
				signerParts.map((p) => p.node),
				signerIndex,
			);

		verifySignatureShare(
			signatureShare,
			this.#storage.verificationShare(groupId),
			signerPart.cl,
			signerPart.r,
		);

		// TODO: burn nonce
		this.#storage.burnNonce(groupId, chunk, offset);
		const submissionId = await this.#coordinator.publishSignatureShare(
			signatureId,
			signingParticipantsHash,
			signingParticipantsProof,
			groupCommitment,
			signerPart.r,
			signatureShare,
			signerPart.l,
		);
		this.#callbacks.onRequestSigned?.(signatureId, signerId, message);
		return submissionId;
	}

	availableNoncesCount(groupId: GroupId, chunk: bigint): bigint {
		try {
			const nonceTree = this.#storage.nonceTree(groupId, chunk);
			return BigInt(nonceTree.leaves.length);
		} catch {
			return 0n;
		}
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
