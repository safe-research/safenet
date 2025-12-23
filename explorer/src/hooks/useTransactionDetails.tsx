import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { loadTransactionProposalDetails, type TransactionDetails } from "@/lib/consensus";
import { useProvider } from "./useProvider";
import { useSettings } from "./useSettings";

export function useTransactionProposalDetails(id: Hex) {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<TransactionDetails | null, Error>({
		queryKey: ["proposalDetails", settings.consensus, id],
		queryFn: () => loadTransactionProposalDetails(provider, settings.consensus, id),
		refetchInterval: 10000,
		initialData: null,
	});
}
