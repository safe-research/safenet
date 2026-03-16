import { useQuery } from "@tanstack/react-query";
import { useSettings } from "@/hooks/useSettings";
import { getConsensusWorker, type LoadTransactionProposalsResult, type TransactionProposal } from "@/lib/consensus";

export function useRecentTransactionProposals(autoRefresh = true) {
	const [settings] = useSettings();
	return useQuery<LoadTransactionProposalsResult, Error, TransactionProposal[]>({
		queryKey: ["recentProposals", settings.consensus, settings.maxBlockRange],
		queryFn: () =>
			getConsensusWorker().loadTransactionProposals({
				rpc: settings.rpc,
				consensus: settings.consensus,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		select: (data) => data.proposals,
		refetchInterval: () => (autoRefresh && settings.refetchInterval > 0 ? settings.refetchInterval : false),
		initialData: { proposals: [], fromBlock: 0n, toBlock: 0n },
	});
}
