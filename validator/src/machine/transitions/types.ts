import type { Address, Hex } from "viem";
import type { SafeTransaction } from "../../consensus/verify/safeTx/schemas.js";
import type { FrostPoint, GroupId, ParticipantId, ProofOfKnowledge, SignatureId } from "../../frost/types.js";

export type NewBlock = {
	id: "block_new";
	block: bigint;
};

export type KeyGenEvent = {
	id: "event_key_gen";
	block: bigint;
	index: number;
	gid: GroupId;
	participants: Hex;
	count: number;
	threshold: number;
	context: Hex;
};

export type KeyGenCommittedEvent = {
	id: "event_key_gen_committed";
	block: bigint;
	index: number;
	gid: GroupId;
	identifier: ParticipantId;
	commitment: ProofOfKnowledge & { c: FrostPoint[] };
	committed: boolean;
};

export type KeyGenSecretSharedEvent = {
	id: "event_key_gen_secret_shared";
	block: bigint;
	index: number;
	gid: GroupId;
	identifier: ParticipantId;
	share: {
		y: FrostPoint;
		f: bigint[];
	};
	shared: boolean;
};

export type KeyGenComplaintSubmittedEvent = {
	id: "event_key_gen_complaint_submitted";
	block: bigint;
	index: number;
	gid: GroupId;
	plaintiff: ParticipantId;
	accused: ParticipantId;
	compromised: boolean;
};

export type KeyGenComplaintResponsedEvent = {
	id: "event_key_gen_complaint_responded";
	block: bigint;
	index: number;
	gid: GroupId;
	plaintiff: ParticipantId;
	accused: ParticipantId;
	secretShare: bigint;
};

export type KeyGenConfirmedEvent = {
	id: "event_key_gen_confirmed";
	block: bigint;
	index: number;
	gid: GroupId;
	identifier: ParticipantId;
	confirmed: boolean;
};

export type NonceCommitmentsHashEvent = {
	id: "event_nonce_commitments_hash";
	block: bigint;
	index: number;
	gid: GroupId;
	identifier: ParticipantId;
	chunk: bigint;
	commitment: Hex;
};

export type SignRequestEvent = {
	id: "event_sign_request";
	block: bigint;
	index: number;
	initiator: Address;
	gid: GroupId;
	message: Hex;
	sid: SignatureId;
	sequence: bigint;
};

export type NonceCommitmentsEvent = {
	id: "event_nonce_commitments";
	block: bigint;
	index: number;
	sid: SignatureId;
	identifier: ParticipantId;
	nonces: {
		d: FrostPoint;
		e: FrostPoint;
	};
};

export type SignatureShareEvent = {
	id: "event_signature_share";
	block: bigint;
	index: number;
	sid: SignatureId;
	identifier: ParticipantId;
	z: bigint;
	root: Hex;
};

export type SignedEvent = {
	id: "event_signed";
	block: bigint;
	index: number;
	sid: SignatureId;
	signature: {
		z: bigint;
		r: FrostPoint;
	};
};

export type EpochProposedEvent = {
	id: "event_epoch_proposed";
	block: bigint;
	index: number;
	activeEpoch: bigint;
	proposedEpoch: bigint;
	rolloverBlock: bigint;
	groupKey: FrostPoint;
};

export type EpochStagedEvent = {
	id: "event_epoch_staged";
	block: bigint;
	index: number;
	activeEpoch: bigint;
	proposedEpoch: bigint;
	rolloverBlock: bigint;
	groupKey: FrostPoint;
};

export type TransactionProposedEvent = {
	id: "event_transaction_proposed";
	block: bigint;
	index: number;
	message: Hex;
	transactionHash: Hex;
	epoch: bigint;
	transaction: SafeTransaction;
};

export type TransactionAttestedEvent = {
	id: "event_transaction_attested";
	block: bigint;
	index: number;
	message: Hex;
};

export type EventTransition =
	| KeyGenEvent
	| KeyGenCommittedEvent
	| KeyGenSecretSharedEvent
	| KeyGenComplaintSubmittedEvent
	| KeyGenComplaintResponsedEvent
	| KeyGenConfirmedEvent
	| NonceCommitmentsHashEvent
	| NonceCommitmentsEvent
	| SignRequestEvent
	| SignatureShareEvent
	| SignedEvent
	| EpochProposedEvent
	| EpochStagedEvent
	| TransactionProposedEvent
	| TransactionAttestedEvent;

export type StateTransition = NewBlock | EventTransition;
