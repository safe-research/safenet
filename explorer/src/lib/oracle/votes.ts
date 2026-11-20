import { type Address, formatLog, type Hex, numberToHex, type PublicClient, parseEventLogs } from "viem";
import { loadChainId } from "@/lib/utils";
import { sentinelOracleAbi, sentinelVoteEventSelectors } from "./abi";
import { oracleRequestId } from "./hashing";

export type SentinelVote =
	| { sentinel: Address; state: "committed" }
	| { sentinel: Address; state: "approved" | "denied"; reason: string };

// `final` is set once the reveal window has closed: the votes can no longer change.
export type SentinelVotes = { votes: SentinelVote[]; final: boolean };

// Per-sentinel breakdown for a `SentinelOracle` request, discovered from indexed `Committed`/
// `Revealed` logs rather than a configured roster — only sentinels who acted appear, ordered by
// their `Committed` log position (first vote first). Callers should only invoke this once
// `loadVotingStatus` has confirmed `kind === "sentinel"`; a generic `IOracle` has no such events.
//
// Logs are searched in the request's own vote window rather than a window counted back from the
// latest block, so votes on requests of any age are found. The request is created in the block its
// proposal was made in (`Consensus.proposeTransaction` posts it), and no vote can be committed or
// revealed after `revealDeadline`.
export const loadSentinelVotes = async ({
	provider,
	oracle,
	consensus,
	epoch,
	safeTxHash,
	oracleData,
	proposedAt,
	revealDeadline,
}: {
	provider: PublicClient;
	oracle: Address;
	consensus: Address;
	epoch: bigint;
	safeTxHash: Hex;
	oracleData: Hex;
	proposedAt: bigint;
	revealDeadline: bigint;
}): Promise<SentinelVotes> => {
	const chainId = await loadChainId(provider);
	const requestId = oracleRequestId({ chainId, consensus, epoch, oracle, safeTxHash, oracleData });
	const latest = await provider.getBlockNumber();
	const fromBlock = proposedAt;
	const toBlock = revealDeadline < latest ? revealDeadline : latest;

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
	return { votes, final: latest > revealDeadline };
};
