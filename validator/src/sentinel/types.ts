import type { Address, Hex } from "viem";

export type CommitApprove = {
	id: "sentinel_commit_approve";
	requestId: Hex;
	bondAmount: bigint;
};

export type CommitDeny = {
	id: "sentinel_commit_deny";
	requestId: Hex;
	bondAmount: bigint;
};

export type SentinelFinalize = {
	id: "sentinel_finalize";
	requestId: Hex;
};

export type SentinelClaim = {
	id: "sentinel_claim";
	requestId: Hex;
};

export type SentinelApproveToken = {
	id: "sentinel_approve_token";
	bondAmount: bigint;
};

export type SentinelAction = CommitApprove | CommitDeny | SentinelFinalize | SentinelClaim | SentinelApproveToken;

export type SentinelActionWithTimeout = SentinelAction & { validUntil: number };

export type SentinelRequestStatus = "preparing" | "pending" | "committed" | "finalized";

export type SentinelRequestState = {
	deadline: bigint;
	status: SentinelRequestStatus;
	approve?: boolean;
};

export type TransactionPayload = { to: Address; value: bigint; data: Hex };

export type SentinelConfig = {
	readonly account: Address;
	readonly bondAmount: bigint;
	readonly consensus: Address;
	readonly oracle: Address;
	readonly chainId: bigint;
	// Voting window in blocks, used as TTL for the preparing state cleanup.
	readonly votingWindow: bigint;
};

export type SentinelStateDiff = {
	request?: [Hex, SentinelRequestState | undefined];
	actions?: SentinelAction[];
};
