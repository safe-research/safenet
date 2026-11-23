import { type Address, formatLog, type Hex, numberToHex, type PublicClient, parseEventLogs } from "viem";
import { sentinelOracleAbi, sentinelVoteEventSelectors } from "./abi";

export type SentinelVote =
	| { sentinel: Address; state: "committed" }
	| { sentinel: Address; state: "approved" | "denied"; reason: string };

// Per-sentinel breakdown for a `SentinelOracle` request, discovered from indexed `Committed`/
// `Revealed` logs rather than a configured roster — only sentinels who acted appear, ordered by
// their `Committed` log position (first vote first). Callers should only invoke this once
// `loadVotingStatus` has confirmed `kind === "sentinel"`; a generic `IOracle` has no such events.
//
// `requestId` is the proposal's own (`TransactionProposal.requestId`), used directly as the event topic.
//
// Logs are searched in the request's own vote window rather than a window counted back from the
// latest block, so votes on requests of any age are found. The request is created in the block its
// proposal was made in (`Consensus.proposeTransaction` posts it), and no vote can be committed or
// revealed after `revealDeadline`. The window is still fetched in `maxBlockRange`-sized pieces, one
// at a time, for RPCs that reject wide `eth_getLogs` spans or rate-limit bursts.
export const loadSentinelVotes = async ({
	provider,
	oracle,
	requestId,
	proposedAt,
	revealDeadline,
	maxBlockRange,
}: {
	provider: PublicClient;
	oracle: Address;
	requestId: Hex;
	proposedAt: bigint;
	revealDeadline: bigint;
	maxBlockRange: bigint;
}): Promise<SentinelVote[]> => {
	const latest = await provider.getBlockNumber();
	const toBlock = revealDeadline < latest ? revealDeadline : latest;

	// `eth_getLogs` directly, in order to filter on the `requestId` topic while still matching
	// either event — `getLogs`'s typed `events` option can't be combined with `args`.
	const rawLogs = [];
	for (let fromBlock = proposedAt; fromBlock <= toBlock; ) {
		// 0 means unlimited: the whole window in one request.
		const chunkEnd = maxBlockRange > 0n ? fromBlock + maxBlockRange - 1n : toBlock;
		const chunkTo = chunkEnd < toBlock ? chunkEnd : toBlock;
		rawLogs.push(
			...(await provider.request({
				method: "eth_getLogs",
				params: [
					{
						address: oracle,
						fromBlock: numberToHex(fromBlock),
						toBlock: numberToHex(chunkTo),
						topics: [sentinelVoteEventSelectors, requestId],
					},
				],
			})),
		);
		fromBlock = chunkTo + 1n;
	}
	const logs = parseEventLogs({
		logs: rawLogs.map((log) => formatLog(log)),
		abi: sentinelOracleAbi,
		strict: true,
	});

	const revealedBySentinel = new Map(
		logs.filter((log) => log.eventName === "Revealed").map((log) => [log.args.sentinel, log.args]),
	);

	return logs
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
};
