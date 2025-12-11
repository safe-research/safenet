import type { Hex } from "viem";
import type { ProtocolAction } from "../consensus/protocol/types.js";
import type { Participant } from "../consensus/storage/types.js";
import type { EpochRolloverPacket } from "../consensus/verify/rollover/schemas.js";
import type { SafeTransactionPacket } from "../consensus/verify/safeTx/schemas.js";
import type { GroupId, ParticipantId, SignatureId } from "../frost/types.js";

export type Optional<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>;

export type RolloverState = Readonly<
	| {
			id: "waiting_for_rollover";
	  }
	| {
			id: "collecting_commitments";
			groupId: GroupId;
			nextEpoch: bigint;
			deadline: bigint;
	  }
	| {
			id: "collecting_shares";
			groupId: GroupId;
			nextEpoch: bigint;
			deadline: bigint;
			lastParticipant?: ParticipantId;
	  }
	| {
			id: "collecting_confirmations";
			groupId: GroupId;
			nextEpoch: bigint;
			deadline: bigint;
			lastParticipant?: ParticipantId;
			sharesFrom: readonly ParticipantId[];
	  }
	| {
			id: "sign_rollover";
			groupId: GroupId;
			nextEpoch: bigint;
			message: Hex;
			responsible: ParticipantId;
	  }
>;

export type BaseSigningState = {
	packet: SafeTransactionPacket | EpochRolloverPacket;
};

export type SigningState = Readonly<
	BaseSigningState &
		(
			| {
					id: "waiting_for_request";
					responsible?: ParticipantId;
					signers: readonly ParticipantId[];
					deadline: bigint;
			  }
			| {
					id: "collect_nonce_commitments";
					signatureId: SignatureId;
					lastSigner?: ParticipantId;
					deadline: bigint;
			  }
			| {
					id: "collect_signing_shares";
					signatureId: SignatureId;
					sharesFrom: readonly ParticipantId[];
					lastSigner?: ParticipantId;
					deadline: bigint;
			  }
			| {
					id: "waiting_for_attestation";
					signatureId: SignatureId;
					responsible?: ParticipantId;
					deadline: bigint;
			  }
		)
>;

export type GroupInfo = {
	groupId: GroupId;
	participantId: ParticipantId;
};

export type ConsensusDiff = {
	groupPendingNonces?: [GroupId, true?];
	activeEpoch?: bigint;
	stagedEpoch?: bigint;
	genesisGroupId?: GroupId;
	epochGroup?: [bigint, GroupInfo];
	signatureIdToMessage?: [SignatureId, Hex?];
};

export type StateDiff = {
	consensus?: ConsensusDiff;
	rollover?: RolloverState;
	signing?: [SignatureId, SigningState?];
	actions?: ProtocolAction[];
};

export type MutableConsensusState = {
	genesisGroupId?: GroupId;
	activeEpoch: bigint;
	stagedEpoch: bigint;
	groupPendingNonces: Record<GroupId, boolean>;
	epochGroups: Record<string, GroupInfo>;
	signatureIdToMessage: Record<SignatureId, Hex>;
};

export type ConsensusState = Readonly<{
	genesisGroupId?: GroupId;
	activeEpoch: bigint;
	stagedEpoch: bigint;
	groupPendingNonces: Readonly<Record<GroupId, boolean>>;
	epochGroups: Readonly<Record<string, Readonly<GroupInfo>>>;
	signatureIdToMessage: Readonly<Record<SignatureId, Hex>>;
}>;

export type MutableMachineStates = {
	rollover: RolloverState;
	signing: Record<SignatureId, SigningState>;
};

export type MachineStates = Readonly<{
	rollover: Readonly<RolloverState>;
	signing: Readonly<Record<SignatureId, Readonly<SigningState>>>;
}>;

export type MachineConfig = {
	defaultParticipants: Participant[];
	genesisSalt: Hex;
	keyGenTimeout: bigint;
	signingTimeout: bigint;
	blocksPerEpoch: bigint;
};
