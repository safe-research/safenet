import { type Address, formatLog, type Hex, numberToHex, type PublicClient, parseEventLogs } from "viem";
import { loadChainId, splitBlockRange } from "@/lib/utils";
import { sentinelOracleAbi, sentinelVoteEventSelectors } from "./abi";
import { oracleRequestId } from "./hashing";

export type SentinelVote =
	| { sentinel: Address; state: "committed" }
	| { sentinel: Address; state: "approved" | "denied"; reason: string };

// `final` is set once the reveal deadline block is finalized: the votes can no longer change, not even
// through a reorg.
export type SentinelVotes = { votes: SentinelVote[]; final: boolean };

// Per-sentinel breakdown for a `SentinelOracle` request, discovered from indexed `Committed`/
// `Revealed` logs rather than a configured roster — only sentinels who acted appear, ordered by
// their `Committed` log position (first vote first). Callers should only invoke this once
// `loadVotingStatus` has confirmed `kind === "sentinel"`; a generic `IOracle` has no such events.
//
// Logs are searched in the request's own vote window rather than a window counted back from the
// latest block, so votes on requests of any age are found. The request is created in the block its
// proposal was made in (`Consensus.proposeTransaction` posts it), and no vote can be committed or
// revealed after `revealDeadline`. The window is still fetched in `maxBlockRange`-sized pieces, for
// RPCs that reject wide `eth_getLogs` spans.
export const loadSentinelVotes = async ({
	provider,
	oracle,
	consensus,
	epoch,
	safeTxHash,
	oracleData,
	proposedAt,
	revealDeadline,
	maxBlockRange,
}: {
	provider: PublicClient;
	oracle: Address;
	consensus: Address;
	epoch: bigint;
	safeTxHash: Hex;
	oracleData: Hex;
	proposedAt: bigint;
	revealDeadline: bigint;
	maxBlockRange: bigint;
}): Promise<SentinelVotes> => {
	const chainId = await loadChainId(provider);
	const requestId = oracleRequestId({ chainId, consensus, epoch, oracle, safeTxHash, oracleData });
	const [latest, finalized] = await Promise.all([provider.getBlockNumber(), loadFinalizedBlock(provider)]);
	const window = { fromBlock: proposedAt, toBlock: revealDeadline < latest ? revealDeadline : latest };

	// `eth_getLogs` directly, in order to filter on the `requestId` topic while still matching
	// either event — `getLogs`'s typed `events` option can't be combined with `args`.
	const rawLogs = await Promise.all(
		splitBlockRange(window, maxBlockRange).map(({ fromBlock, toBlock }) =>
			provider.request({
				method: "eth_getLogs",
				params: [
					{
						address: oracle,
						fromBlock: numberToHex(fromBlock),
						toBlock: numberToHex(toBlock),
						topics: [sentinelVoteEventSelectors, requestId],
					},
				],
			}),
		),
	);
	const logs = parseEventLogs({
		logs: rawLogs.flat().map((log) => formatLog(log)),
		abi: sentinelOracleAbi,
		strict: true,
	});

	const revealedBySentinel = new Map(
		logs.filter((log) => log.eventName === "Revealed").map((log) => [log.args.sentinel, log.args]),
	);

	const votes = logs
		.filter((log) => log.eventName === "Committed")
		.sort((left, right) => {
			if (left.blockNumber !== right.blockNumber) {
				return left.blockNumber < right.blockNumber ? -1 : 1;
			}
			return left.logIndex - right.logIndex;
		})
		.map(({ args: { sentinel } }): SentinelVote => {
			const revealed = revealedBySentinel.get(sentinel);
			if (!revealed) {
				return { sentinel, state: "committed" };
			}
			return { sentinel, state: revealed.approved ? "approved" : "denied", reason: revealed.reason };
		});
	return { votes, final: finalized !== null && finalized >= revealDeadline };
};

// The latest finalized block, or null when the RPC doesn't support the `finalized` tag. Votes are then
// never considered final, so they keep being polled.
const loadFinalizedBlock = async (provider: PublicClient): Promise<bigint | null> => {
	try {
		return (await provider.getBlock({ blockTag: "finalized" })).number;
	} catch {
		return null;
	}
};
