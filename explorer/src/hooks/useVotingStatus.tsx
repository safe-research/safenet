import { useQuery } from "@tanstack/react-query";
import type { Address, Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getOracleWorker, type VotingStatus } from "@/lib/oracle";

export function useVotingStatus(oracle: Address, requestId: Hex) {
	const [settings] = useSettings();
	return useQuery<VotingStatus, Error>({
		queryKey: ["votingStatus", settings.rpc, oracle, requestId, settings.maxBlockRange],
		queryFn: () =>
			getOracleWorker().loadVotingStatus({
				rpc: settings.rpc,
				oracle,
				requestId,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
	});
}
