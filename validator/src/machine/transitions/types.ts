import type { Address, Hex } from "viem";
import type { MetaTransaction } from "../../consensus/verify/safeTx/schemas.js";
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
	count: bigint;
	threshold: bigint;
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
	completed: boolean;
};

export type KeyGenConfirmedEvent = {
	id: "event_key_gen_confirmed";
	block: bigint;
	index: number;
	gid: GroupId;
	identifier: ParticipantId;
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
	transaction: MetaTransaction;
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
