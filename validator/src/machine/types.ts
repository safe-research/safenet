import type { Hex } from "viem";
import type { ProtocolAction } from "../consensus/protocol/types.js";
import type { Participant } from "../consensus/storage/types.js";
import type { GroupId, ParticipantId, SignatureId } from "../frost/types.js";

export type RolloverState =
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
			id: "sign_rollover";
			groupId: GroupId;
			nextEpoch: bigint;
			message: Hex;
			responsible: ParticipantId;
	  };

export type SigningState =
	| {
			id: "waiting_for_request";
			responsible: ParticipantId | undefined;
			signers: ParticipantId[];
			deadline: bigint;
	  }
	| {
			id: "collect_nonce_commitments";
			lastSigner: ParticipantId | undefined;
			deadline: bigint;
	  }
	| {
			id: "collect_signing_shares";
			sharesFrom: ParticipantId[];
			lastSigner: ParticipantId | undefined;
			deadline: bigint;
	  }
	| {
			id: "waiting_for_attestation";
			responsible: ParticipantId | undefined;
			deadline: bigint;
	  };

export type StateTransition =
	| {
			type: "block";
			block: bigint;
	  }
	| {
			type: "event";
			block: bigint;
			index: number;
			eventName: string;
			eventArgs: unknown;
	  };

export type StateDiff = {
	rollover?: RolloverState;
	signing?: [SignatureId, SigningState?];
	actions?: ProtocolAction[];
};

export type ConsensusState = {
	genesisGroupId?: GroupId;
	epochGroups: Map<bigint, GroupId>;
	groupPendingNonces: Set<GroupId>;
	activeEpoch: bigint;
	stagedEpoch: bigint;
	messageSignatureRequests: Map<Hex, SignatureId>;
	transactionProposalInfo: Map<Hex, { epoch: bigint; transactionHash: Hex }>;
};

export type MachineStates = {
	rollover: RolloverState;
	signing: Map<SignatureId, SigningState>;
};

export type MachineConfig = {
	defaultParticipants: Participant[];
	keyGenTimeout: bigint;
	signingTimeout: bigint;
	blocksPerEpoch: bigint;
};
