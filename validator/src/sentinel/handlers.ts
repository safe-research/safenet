import type { Hex } from "viem";
import { isAddressEqual } from "viem";
import { oracleTxProposalHash } from "../consensus/verify/oracleTx/hashing.js";
import type { OracleTransactionProposedEvent } from "../machine/transitions/types.js";
import type { Logger } from "../utils/logging.js";
import type { Detector } from "./detector.js";
import type {
	SentinelCommittedTransition,
	SentinelDisputeResolvedTransition,
	SentinelNewRequestTransition,
	SentinelOracleResultTransition,
} from "./transitions.js";
import type { SentinelConfig, SentinelRequestState, SentinelStateDiff } from "./types.js";

export function handleOracleTransactionProposed(
	transition: OracleTransactionProposedEvent,
	config: SentinelConfig,
	detector: Detector,
): SentinelStateDiff {
	// Only handle requests from the configured oracle
	if (!isAddressEqual(transition.oracle, config.oracle)) return {};

	const requestId = oracleTxProposalHash({
		domain: { chain: config.chainId, consensus: config.consensus },
		proposal: { epoch: transition.epoch, oracle: transition.oracle, safeTxHash: transition.safeTxHash },
	});
	const approve = detector(transition.transaction);
	return {
		request: [requestId, { deadline: transition.block + config.votingWindow, status: "preparing", approve }],
	};
}

export function handleNewRequest(
	requests: ReadonlyMap<Hex, SentinelRequestState>,
	transition: SentinelNewRequestTransition,
	config: SentinelConfig,
	logger: Logger,
): SentinelStateDiff {
	const existing = requests.get(transition.requestId);
	if (existing !== undefined && existing.status !== "preparing") return {};
	const approve = existing?.approve ?? true;
	logger.info("SentinelService: committing vote", {
		requestId: transition.requestId,
		approve,
		hasTxPayload: existing !== undefined,
	});
	return {
		request: [transition.requestId, { deadline: transition.deadline, status: "pending" }],
		actions: [
			{ id: "sentinel_approve_token", bondTarget: transition.bondTarget },
			approve
				? { id: "sentinel_commit_approve", requestId: transition.requestId, bondAmount: config.bondAmount }
				: { id: "sentinel_commit_deny", requestId: transition.requestId, bondAmount: config.bondAmount },
		],
	};
}

export function handleCommitted(
	requests: ReadonlyMap<Hex, SentinelRequestState>,
	transition: SentinelCommittedTransition,
	config: SentinelConfig,
): SentinelStateDiff {
	if (!isAddressEqual(config.account, transition.sentinel)) return {};
	const existing = requests.get(transition.requestId);
	if (existing === undefined || existing.status !== "pending") return {};
	return {
		request: [transition.requestId, { deadline: existing.deadline, status: "committed" }],
	};
}

export function handleResolved(
	requests: ReadonlyMap<Hex, SentinelRequestState>,
	transition: SentinelOracleResultTransition | SentinelDisputeResolvedTransition,
): SentinelStateDiff {
	if (!requests.has(transition.requestId)) return {};
	return {
		request: [transition.requestId, undefined],
		actions: [{ id: "sentinel_claim", requestId: transition.requestId }],
	};
}

export function handleBlockAdvance(
	requests: ReadonlyMap<Hex, SentinelRequestState>,
	blockNumber: bigint,
): SentinelStateDiff[] {
	const diffs: SentinelStateDiff[] = [];
	for (const [requestId, { deadline, status }] of requests) {
		if (blockNumber <= deadline) continue;
		if (status === "committed") {
			// Transition to finalized + queue the finalize action (emitted at most once)
			diffs.push({
				request: [requestId, { deadline, status: "finalized" }],
				actions: [{ id: "sentinel_finalize", requestId }],
			});
		} else if (status === "pending" || status === "preparing") {
			// Drop requests where the voting window passed or no matching NewRequest arrived
			diffs.push({ request: [requestId, undefined] });
		}
		// "finalized" status: waiting for OracleResult — no action needed
	}
	return diffs;
}
