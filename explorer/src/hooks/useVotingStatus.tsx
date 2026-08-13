import { useQuery } from "@tanstack/react-query";
import type { Address, Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getOracleWorker, type VotingStatus } from "@/lib/oracle";

export function useVotingStatus(oracle: Address, epoch: bigint, safeTxHash: Hex, oracleData: Hex) {
	const [settings] = useSettings();
	return useQuery<VotingStatus, Error>({
		queryKey: [
			"votingStatus",
			settings.rpc,
			oracle,
			settings.consensus,
			epoch.toString(),
			safeTxHash,
			oracleData,
			settings.maxBlockRange,
		],
		queryFn: () =>
			getOracleWorker().loadVotingStatus({
				rpc: settings.rpc,
				oracle,
				consensus: settings.consensus,
				epoch,
				safeTxHash,
				oracleData,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialData: null,
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
	});
}
