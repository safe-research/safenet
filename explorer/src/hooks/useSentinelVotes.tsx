import { useQuery } from "@tanstack/react-query";
import type { Address, Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getOracleWorker, type SentinelVote } from "@/lib/oracle";

export function useSentinelVotes(oracle: Address, requestId: Hex) {
	const [settings] = useSettings();
	return useQuery<SentinelVote[], Error>({
		queryKey: ["sentinelVotes", settings.rpc, oracle, requestId, settings.maxBlockRange],
		queryFn: () =>
			getOracleWorker().loadSentinelVotes({
				rpc: settings.rpc,
				oracle,
				requestId,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialData: [],
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
	});
}
