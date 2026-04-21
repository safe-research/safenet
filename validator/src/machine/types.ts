import type { Address, Hex } from "viem";
import type { ProtocolAction } from "../consensus/protocol/types.js";
import type { OracleTransactionPacket } from "../consensus/verify/oracleTx/schemas.js";
import type { EpochRolloverPacket } from "../consensus/verify/rollover/schemas.js";
import type { SafeTransactionPacket } from "../consensus/verify/safeTx/schemas.js";
import type { GroupId, SignatureId } from "../frost/types.js";
import type { ParticipantInfo } from "../types/interfaces.js";

export type Optional<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>;

export type Complaints = {
	unresponded: number;
	total: number;
};

export type RolloverState = Readonly<
	| {
			id: "waiting_for_genesis";
	  }
	| {
			id: "skip_genesis";
	  }
	| {
			id: "epoch_skipped";
			nextEpoch: bigint;
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
			complaints: Readonly<Record<Address, Readonly<Complaints>>>;
			sharesFrom: readonly Address[];
			lastParticipant?: Address;
	  }
	| {
			id: "collecting_confirmations";
			groupId: GroupId;
			nextEpoch: bigint;
			complaintDeadline: bigint;
			responseDeadline: bigint;
			deadline: bigint;
			lastParticipant?: Address;
			complaints: Readonly<Record<Address, Readonly<Complaints>>>;
			sharesFrom: readonly Address[];
			confirmationsFrom: readonly Address[];
	  }
	| {
			id: "sign_rollover";
			groupId: GroupId;
			nextEpoch: bigint;
			message: Hex;
	  }
	| {
			id: "epoch_staged";
			nextEpoch: bigint;
	  }
>;

export type BaseSigningState = {
	packet: SafeTransactionPacket | EpochRolloverPacket | OracleTransactionPacket;
};

export type SigningState = Readonly<
	BaseSigningState &
		(
			| {
					id: "waiting_for_request";
					responsible?: Address;
					signers: readonly Address[];
					deadline: bigint;
			  }
			| {
					id: "collect_nonce_commitments";
					signatureId: SignatureId;
					lastSigner?: Address;
					deadline: bigint;
			  }
			| {
					id: "collect_signing_shares";
					signatureId: SignatureId;
					sharesFrom: readonly Address[];
					lastSigner?: Address;
					deadline: bigint;
			  }
			| {
					id: "waiting_for_attestation";
					signatureId: SignatureId;
					responsible?: Address;
					deadline: bigint;
			  }
			| {
					id: "wait_for_oracle";
					oracle: Address;
					signers: readonly Address[];
					deadline: bigint;
			  }
		)
>;

export type ConsensusDiff = {
	groupPendingNonces?: [GroupId, true?];
	activeEpoch?: bigint;
	genesisGroupId?: GroupId;
	epochGroup?: [bigint, GroupId];
	removeEpochGroups?: bigint[];
	signatureIdToMessage?: [SignatureId, Hex?];
};

export type StateDiff = {
	consensus?: ConsensusDiff;
	rollover?: RolloverState;
	signing?: [Hex, SigningState?];
	actions?: ProtocolAction[];
};

export type MutableConsensusState = {
	genesisGroupId?: GroupId;
	activeEpoch: bigint;
	groupPendingNonces: Record<GroupId, boolean>;
	epochGroups: Record<string, GroupId>;
	signatureIdToMessage: Record<SignatureId, Hex>;
};

export type ConsensusState = Readonly<{
	genesisGroupId?: GroupId;
	activeEpoch: bigint;
	groupPendingNonces: Readonly<Record<GroupId, boolean>>;
	epochGroups: Readonly<Record<string, GroupId>>;
	signatureIdToMessage: Readonly<Record<SignatureId, Hex>>;
}>;

export type MutableMachineStates = {
	rollover: RolloverState;
	signing: Record<Hex, SigningState>;
};

export type MachineStates = Readonly<{
	rollover: Readonly<RolloverState>;
	signing: Readonly<Record<Hex, Readonly<SigningState>>>;
}>;

export type MachineConfig = {
	account: Address;
	participantsInfo: ParticipantInfo[];
	genesisSalt: Hex;
	keyGenTimeout: bigint;
	signingTimeout: bigint;
	blocksPerEpoch: bigint;
	allowedOracles: readonly Address[];
	oracleTimeout: bigint;
};
