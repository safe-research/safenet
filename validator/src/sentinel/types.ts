import type { Hex } from "viem";

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
	bondTarget: bigint;
};

export type SentinelAction = CommitApprove | CommitDeny | SentinelFinalize | SentinelClaim | SentinelApproveToken;

export type SentinelActionWithTimeout = SentinelAction & { validUntil: number };

export type SentinelRequestStatus = "pending" | "committed" | "finalized";

export type SentinelRequestState = {
	deadline: bigint;
	status: SentinelRequestStatus;
};

export type SentinelStateDiff = {
	request?: [Hex, SentinelRequestState | undefined];
	actions?: SentinelAction[];
};
