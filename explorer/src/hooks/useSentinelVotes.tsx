import { useQuery } from "@tanstack/react-query";
import type { Address, Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getOracleWorker, type SentinelVote } from "@/lib/oracle";

// `revealDeadline` should stay undefined until the caller knows the oracle is `SentinelOracle`-shaped
// (i.e. from `useVotingStatus(...).data` with `kind === "sentinel"`) — a generic `IOracle` has no
// `Committed`/`Revealed` events to read, so querying it is pure wasted RPC traffic.
export function useSentinelVotes(
	oracle: Address,
	requestId: Hex,
	proposedAt: bigint,
	revealDeadline: bigint | undefined,
) {
	const [settings] = useSettings();
	return useQuery<SentinelVote[], Error>({
		queryKey: [
			"sentinelVotes",
			settings.rpc,
			oracle,
			requestId,
			proposedAt.toString(),
			revealDeadline?.toString(),
			settings.maxBlockRange,
		],
		queryFn: () =>
			getOracleWorker().loadSentinelVotes({
				rpc: settings.rpc,
				oracle,
				requestId,
				proposedAt,
				revealDeadline: revealDeadline ?? 0n,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialData: [],
		enabled: revealDeadline !== undefined,
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
	});
}
