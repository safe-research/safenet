import { type Address, formatLog, type Hex, numberToHex, type PublicClient, parseEventLogs } from "viem";
import { getBlockRange, loadChainId } from "@/lib/utils";
import { sentinelOracleAbi, sentinelVoteEventSelectors } from "./abi";
import { oracleRequestId } from "./hashing";

export type SentinelVote =
	| { sentinel: Address; state: "committed" }
	| { sentinel: Address; state: "approved" | "denied"; reason: string };

// Per-sentinel breakdown for a `SentinelOracle` request, discovered from indexed `Committed`/
// `Revealed` logs rather than a configured roster — only sentinels who acted appear, ordered by
// their `Committed` log position (first vote first). Callers should only invoke this once
// `loadVotingStatus` has confirmed `kind === "sentinel"`; a generic `IOracle` has no such events.
export const loadSentinelVotes = async ({
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
}): Promise<SentinelVote[]> => {
	const chainId = await loadChainId(provider);
	const requestId = oracleRequestId({ chainId, consensus, epoch, oracle, safeTxHash, oracleData });
	const { fromBlock, toBlock } = await getBlockRange(provider, maxBlockRange);

	// A single `eth_getLogs` here, in order to filter on the `requestId` topic while still
	// matching either event — `getLogs`'s typed `events` option can't be combined with `args`.
	const rawLogs = await provider.request({
		method: "eth_getLogs",
		params: [
			{
				address: oracle,
				fromBlock: numberToHex(fromBlock),
				toBlock: numberToHex(toBlock),
				topics: [sentinelVoteEventSelectors, requestId],
			},
		],
	});
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
