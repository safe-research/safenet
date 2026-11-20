import { useQuery } from "@tanstack/react-query";
import type { Address, Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getOracleWorker, type SentinelVotes } from "@/lib/oracle";

// `revealDeadline` should stay undefined until the caller knows the oracle is `SentinelOracle`-shaped
// (i.e. from `useVotingStatus(...).data` with `kind === "sentinel"`) — a generic `IOracle` has no
// `Committed`/`Revealed` events to read, so querying it is pure wasted RPC traffic.
export function useSentinelVotes(
	oracle: Address,
	epoch: bigint,
	safeTxHash: Hex,
	oracleData: Hex,
	proposedAt: bigint,
	revealDeadline: bigint | undefined,
) {
	const [settings] = useSettings();
	return useQuery<SentinelVotes, Error>({
		queryKey: [
			"sentinelVotes",
			settings.rpc,
			oracle,
			settings.consensus,
			epoch.toString(),
			safeTxHash,
			oracleData,
			proposedAt.toString(),
			revealDeadline?.toString(),
			settings.maxBlockRange,
		],
		queryFn: () =>
			getOracleWorker().loadSentinelVotes({
				rpc: settings.rpc,
				oracle,
				consensus: settings.consensus,
				epoch,
				safeTxHash,
				oracleData,
				proposedAt,
				revealDeadline: revealDeadline ?? 0n,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialData: { votes: [], final: false },
		enabled: revealDeadline !== undefined,
		// Once the reveal deadline block is finalized the votes can no longer change, not even through a reorg.
		refetchInterval: (query) =>
			!query.state.data?.final && settings.refetchInterval > 0 ? settings.refetchInterval : false,
	});
}
