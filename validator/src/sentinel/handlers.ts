import type { Hex } from "viem";
import { decodeAbiParameters, isAddressEqual } from "viem";
import { oracleTxProposalHash } from "../consensus/verify/oracleTx/hashing.js";
import type { OracleTransactionProposedEvent } from "../machine/transitions/types.js";
import type { Logger } from "../utils/logging.js";
import type { Detector } from "./detector.js";
import type {
	SentinelCommittedTransition,
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
	if (existing === undefined || existing.status !== "preparing") return {};
	const approve = existing.approve;
	logger.info("SentinelService: committing vote", {
		requestId: transition.requestId,
		approve,
		hasTxPayload: existing !== undefined,
	});
	return {
		request: [transition.requestId, { deadline: transition.deadline, status: "pending", approve }],
		actions: [
			{ id: "sentinel_approve_token", bondAmount: transition.bondTarget },
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
		request: [transition.requestId, { deadline: existing.deadline, status: "committed", approve: existing.approve }],
	};
}

export function handleResolved(
	requests: ReadonlyMap<Hex, SentinelRequestState>,
	transition: SentinelOracleResultTransition,
): SentinelStateDiff {
	const existing = requests.get(transition.requestId);
	if (existing === undefined) return {};
	// Only claim if we committed on-chain; drop silently otherwise (e.g. commit tx never confirmed).
	if (existing.status !== "committed" && existing.status !== "finalized") {
		return { request: [transition.requestId, undefined] };
	}
	// result encodes SentinelOracleRequest.ResolveReason (uint8): TIMEOUT = 2.
	// On timeout every participant gets their bond back regardless of how they voted.
	const [reason] = decodeAbiParameters([{ type: "uint8" }], transition.result);
	const voteWon = reason === 2 || transition.approved === existing.approve;
	return {
		request: [transition.requestId, undefined],
		actions: voteWon ? [{ id: "sentinel_claim", requestId: transition.requestId }] : undefined,
	};
}

export function handleBlockAdvance(
	requests: ReadonlyMap<Hex, SentinelRequestState>,
	blockNumber: bigint,
	config: SentinelConfig,
): SentinelStateDiff[] {
	const diffs: SentinelStateDiff[] = [];
	for (const [requestId, state] of requests) {
		if (blockNumber <= state.deadline) continue;
		if (state.status === "committed") {
			// Transition to finalized; deadline is reset to give time for OracleResult
			diffs.push({
				request: [
					requestId,
					{ status: "finalized", deadline: blockNumber + config.votingWindow, approve: state.approve },
				],
				actions: [{ id: "sentinel_finalize", requestId }],
			});
		} else if (state.status === "pending" || state.status === "preparing" || state.status === "finalized") {
			diffs.push({ request: [requestId, undefined] });
		}
	}
	return diffs;
}
