import type { Address, Hex } from "viem";
import type {
	FrostPoint,
	GroupId,
	ParticipantId,
	ProofOfAttestationParticipation,
	ProofOfKnowledge,
	SignatureId,
} from "../../frost/types.js";
import type { PublicNonceCommitments } from "../signing/nonces.js";

export type ShieldnetProtocol = {
	chainId(): bigint;
	consensus(): Address;
	coordinator(): Address;
	process(action: ProtocolAction): void;
};

export type RegisterNonceCommitments = {
	id: "sign_register_nonce_commitments";
	groupId: GroupId;
	nonceCommitmentsHash: Hex;
};

export type RevealNonceCommitments = {
	id: "sign_reveal_nonce_commitments";
	signatureId: SignatureId;
	nonceCommitments: PublicNonceCommitments;
	nonceProof: Hex[];
};

export type PublishSignatureShare = {
	id: "sign_publish_signature_share";
	signatureId: SignatureId;
	signersRoot: Hex;
	signersProof: Hex[];
	groupCommitment: FrostPoint;
	commitmentShare: FrostPoint;
	signatureShare: bigint;
	lagrangeCoefficient: bigint;
	callbackContext?: Hex;
};

export type SigningAction =
	| RegisterNonceCommitments
	| RevealNonceCommitments
	| PublishSignatureShare;

export type StartKeyGen = {
	id: "key_gen_start";
	participants: Hex;
	count: bigint;
	threshold: bigint;
	context: Hex;
	participantId: ParticipantId;
	commitments: FrostPoint[];
	pok: ProofOfKnowledge;
	poap: ProofOfAttestationParticipation;
};

export type PublishSecretShares = {
	id: "key_gen_publish_secret_shares";
	groupId: GroupId;
	verificationShare: FrostPoint;
	shares: bigint[];
	callbackContext?: Hex;
};
export type KeyGenAction = StartKeyGen | PublishSecretShares;

export type ProtocolAction = KeyGenAction | SigningAction;
