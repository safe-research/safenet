import { encodePacked, Hex, keccak256 } from "viem";
import { GroupId, SignatureId } from "../../frost/types.js";
import { KeygenInfo } from "../client.js";
import { createNonceTree, NonceTree, PublicNonceCommitments } from "./nonces.js";
import { SigningCoordinator } from "../types.js";
import { generateMerkleProof } from "../merkle.js";

const SEQUENCE_CHUNK_SIZE = 1024n

type SignatureRequest = {
    message: Hex
    signerNonceCommitments: Map<bigint, PublicNonceCommitments>
    signers: bigint[]
}

const checkInformationComplete = (
    signers: bigint[],
    information: Map<bigint, unknown>,
): boolean => {
    for (const signer of signers) {
        if (!information.has(signer)) {
            return false;
        }
    }
    return true;
};

class SigningClient {

    #coordinator: SigningCoordinator
    #keyGenInfo = new Map<GroupId, KeygenInfo>();
    #nonceCommits = new Map<Hex, NonceTree>();
    #chunkNonces = new Map<Hex, Hex>();
    #signingRequests = new Map<Hex, SignatureRequest>();

    constructor(coordinator: SigningCoordinator) {
        this.#coordinator = coordinator;
    }

    async commitNonces(groupId: GroupId) {
        const info = this.#keyGenInfo.get(groupId)
        if (info?.signingShare == undefined) 
            throw Error(`No info for ${groupId}`)
        const nonceTree = createNonceTree(info?.signingShare, SEQUENCE_CHUNK_SIZE)
        this.#nonceCommits.set(nonceTree.root, nonceTree)
        await this.#coordinator.publishNonceCommitmentsHash(
            groupId,
            nonceTree.root
        )
    }

    async handleNonceCommitmentsHash(
        groupId: GroupId,
        participantIndex: bigint,
        nonceCommitmentsHash: Hex,
        chunk: bigint
    ) {
        const info = this.#keyGenInfo.get(groupId)
        // Only link own nonce commitments
        if (info?.participantIndex != participantIndex) 
            return
        const chunkId = keccak256(encodePacked(["bytes32", "uint256"], [groupId, chunk]))
        if (this.#chunkNonces.has(chunkId))
            throw Error(`Chunk ${groupId}:${chunk} has already be linked`)
        this.#chunkNonces.set(chunkId, nonceCommitmentsHash)
    }

    async handleSignatureRequest(
        groupId: GroupId,
        signatureId: SignatureId,
        message: Hex,
        sequence: bigint
    ) {
        const info = this.#keyGenInfo.get(groupId)
        if (info == undefined) 
            throw Error(`No info for ${groupId}`)
        if (this.#signingRequests.has(signatureId))
            throw Error(`Already handled signature request: ${signatureId}`)
        // TODO: check if we really want to sign the message

        const chunk = sequence / SEQUENCE_CHUNK_SIZE;
        const chunkId = keccak256(encodePacked(["bytes32", "uint256"], [groupId, chunk]))

        const nonceCommitmentsHash = this.#chunkNonces.get(chunkId)
        if (nonceCommitmentsHash == undefined) 
            throw Error(`Unknown chunk ${chunk} for group ${groupId}`)

        const nonceTree = this.#nonceCommits.get(nonceCommitmentsHash)
        if (nonceTree == undefined) 
            throw Error(`Unknown nonce commitments hash: ${nonceCommitmentsHash}`)

        const nonceOffset = Number(sequence % SEQUENCE_CHUNK_SIZE)
        const nonceCommitments = nonceTree.commitments[nonceOffset]
        const nonceProof = generateMerkleProof(nonceTree.leaves, nonceOffset)

        const signerNonceCommitments = new Map<bigint, PublicNonceCommitments>()
        // Set own nonce commitments
        signerNonceCommitments.set(info.participantIndex, nonceCommitments)
        this.#signingRequests.set(signatureId, {
            message,
            signerNonceCommitments,
            signers: info.participants.map((p) => p.index)
        })
        await this.#coordinator.publishNonceCommitments(
            signatureId,
            nonceCommitments,
            nonceProof
        )
    }

    async handleNonceCommitments(
        signatureId: SignatureId,
        peerIndex: bigint,
        nonceCommitments: PublicNonceCommitments
    ) {
        const signatureRequest = this.#signingRequests.get(signatureId)
        if (signatureRequest === undefined)
            throw Error(`Unknown signature request: ${signatureId}`)

        if (signatureRequest.signerNonceCommitments.has(peerIndex))
            throw Error(`Already registered nonce commitments for ${peerIndex}`)

        signatureRequest.signerNonceCommitments.set(peerIndex, nonceCommitments)

        if(checkInformationComplete(
            signatureRequest.signers, 
            signatureRequest.signerNonceCommitments)
        ) {
            await this.submitSignature(signatureId, signatureRequest)
        }
    }

    private async submitSignature(
        signatureId: SignatureId,
        signatureRequest: SignatureRequest
    ) {

    }
}