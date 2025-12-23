import { useQuery } from "@tanstack/react-query";
import { loadRecentTransactionProposals, type TransactionProposal } from "@/lib/consensus";
import { useProvider } from "./useProvider";
import { useSettings } from "./useSettings";

export function useRecentTransactionProposals() {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<TransactionProposal[], Error>({
		queryKey: ["recentProposals", settings.consensus],
		queryFn: () => loadRecentTransactionProposals(provider, settings.consensus),
		refetchInterval: 10000,
		initialData: [],
	});
}
