import { type Address, getAbiItem, type Hex, type PublicClient } from "viem";
import { getBlockRange, loadChainId, mostRecentFirst } from "@/lib/utils";
import { oracleAbi, sentinelOracleAbi } from "./abi";
import { oracleRequestId } from "./hashing";

// `NONE` (ordinal 0) is a zero-value sentinel `SentinelOracleRequest.State` uses to mean "request
// was never created" — a real `getRequest` response never reports it as a `VotingStatus.state`
// (see the `progress.state === 0` check below), so it's excluded from the public type.
const sentinelRequestStates = [
	"NONE",
	"PENDING",
	"FROZEN",
	"RESOLVED_APPROVED",
	"RESOLVED_DENIED",
	"TIMED_OUT",
] as const;

export type SentinelRequestState = Exclude<(typeof sentinelRequestStates)[number], "NONE">;

export type VotingStatus =
	| {
			kind: "sentinel";
			state: SentinelRequestState;
			approveCount: bigint;
			denyCount: bigint;
			// No vote was revealed and none can be any more.
			noVotes: boolean;
	  }
	| { kind: "generic"; approved: boolean }
	| null;

export const loadVotingStatus = async ({
	provider,
	oracle,
	consensus,
	epoch,
	safeTxHash,
	oracleData,
	maxBlockRange,
}: {
	provider: PublicClient;
	oracle: Address;
	consensus: Address;
	epoch: bigint;
	safeTxHash: Hex;
	oracleData: Hex;
	maxBlockRange: bigint;
}): Promise<VotingStatus> => {
	const chainId = await loadChainId(provider);
	const requestId = oracleRequestId({ chainId, consensus, epoch, oracle, safeTxHash, oracleData });
	// Read before the request: once this block is past a deadline, the counts that deadline closes are final.
	const block = await provider.getBlockNumber();

	try {
		const { terms, progress } = await provider.readContract({
			address: oracle,
			abi: sentinelOracleAbi,
			functionName: "getRequest",
			args: [requestId],
		});
		if (progress.state === 0) return null;
		const state = sentinelRequestStates[progress.state] as SentinelRequestState;
		// `SentinelOracleRequest.finalize`'s guard and outcome: a `PENDING` request with nothing revealed
		// and its window closed can only end `TIMED_OUT`, and a `TIMED_OUT` one without votes ended that
		// way (a dispute that ended without a ruling has votes on both sides).
		const noVotes =
			(state === "PENDING" &&
				progress.revealedCount === 0 &&
				(block > terms.revealDeadline || (progress.committedCount === 0 && block > terms.commitDeadline))) ||
			(state === "TIMED_OUT" && progress.approveSentinelCount === 0 && progress.denySentinelCount === 0);
		return {
			kind: "sentinel",
			state,
			approveCount: BigInt(progress.approveSentinelCount),
			denyCount: BigInt(progress.denySentinelCount),
			noVotes,
		};
	} catch {
		const { fromBlock, toBlock } = await getBlockRange(provider, maxBlockRange, block);
		const logs = mostRecentFirst(
			await provider.getLogs({
				address: oracle,
				event: getAbiItem({ abi: oracleAbi, name: "OracleResult" }),
				args: { requestId },
				fromBlock,
				toBlock,
				strict: true,
			}),
		);
		const result = logs.at(0);
		return result ? { kind: "generic", approved: result.args.approved } : null;
	}
};
