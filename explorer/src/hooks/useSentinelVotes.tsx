import { useQuery } from "@tanstack/react-query";
import type { Address, Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getOracleWorker, type SentinelVote } from "@/lib/oracle";

// `enabled` should be false until the caller knows the oracle is `SentinelOracle`-shaped
// (i.e. `useVotingStatus(...).data?.kind === "sentinel"`) — a generic `IOracle` has no
// `Committed`/`Revealed` events to read, so querying it is pure wasted RPC traffic.
export function useSentinelVotes(oracle: Address, epoch: bigint, safeTxHash: Hex, enabled = false) {
	const [settings] = useSettings();
	return useQuery<SentinelVote[], Error>({
		queryKey: [
			"sentinelVotes",
			settings.rpc,
			oracle,
			settings.consensus,
			epoch.toString(),
			safeTxHash,
			settings.maxBlockRange,
		],
		queryFn: () =>
			getOracleWorker().loadSentinelVotes({
				rpc: settings.rpc,
				oracle,
				consensus: settings.consensus,
				epoch,
				safeTxHash,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialData: [],
		enabled,
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
	});
}
