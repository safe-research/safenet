import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getConsensusWorker, type SafeTransaction } from "@/lib/consensus";
import { loadSafeTransactionDetails } from "@/lib/safe/service";

type SafeTransactionSource = { data: SafeTransaction; fromSafeApi: boolean; submittedAtSeconds: number | null };

export function useSafeTransactionDetails(
	chainId: bigint,
	safeTxHash: Hex,
): { data: SafeTransaction | null; fromSafeApi: boolean; submittedAtSeconds: number | null; isFetching: boolean } {
	const [settings] = useSettings();
	const query = useQuery<SafeTransactionSource | null, Error>({
		queryKey: ["safeTxDetails", chainId.toString(), safeTxHash, settings.consensus, settings.maxBlockRange],
		queryFn: async () => {
			const apiData = await loadSafeTransactionDetails(chainId, safeTxHash);
			if (apiData !== null) {
				return { data: apiData.transaction, fromSafeApi: true, submittedAtSeconds: apiData.submittedAtSeconds };
			}
			const onChainData = await getConsensusWorker().loadProposedSafeTransaction({
				rpc: settings.rpc,
				consensus: settings.consensus,
				safeTxHash,
				maxBlockRange: BigInt(settings.maxBlockRange),
			});
			return onChainData !== null ? { data: onChainData, fromSafeApi: false, submittedAtSeconds: null } : null;
		},
		initialData: null,
	});
	return {
		data: query.data?.data ?? null,
		fromSafeApi: query.data?.fromSafeApi ?? false,
		submittedAtSeconds: query.data?.submittedAtSeconds ?? null,
		isFetching: query.isFetching,
	};
}
