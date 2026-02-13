import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { loadProposalsForTransaction, type TransactionProposal } from "@/lib/consensus";
import { useProvider } from "./useProvider";
import { useSettings } from "./useSettings";

export function useProposalsForTransaction(proposalTxHash: Hex) {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<TransactionProposal[], Error>({
		queryKey: ["proposalsForTransactionHash", settings.consensus, proposalTxHash],
		queryFn: () => loadProposalsForTransaction(provider, settings.consensus, proposalTxHash),
		initialData: [],
		refetchInterval: 10000,
	});
}
