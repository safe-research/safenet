import { useQuery } from "@tanstack/react-query";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import {
	type LoadTransactionProposalsResult,
	loadTransactionProposals,
	type TransactionProposal,
} from "@/lib/consensus";

export function useRecentTransactionProposals() {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<LoadTransactionProposalsResult, Error, TransactionProposal[]>({
		queryKey: ["recentProposals", settings.consensus, settings.maxBlockRange],
		queryFn: () =>
			loadTransactionProposals({
				provider,
				consensus: settings.consensus,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		select: (data) => data.proposals,
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
		initialData: { proposals: [], fromBlock: 0n, toBlock: 0n },
	});
}
