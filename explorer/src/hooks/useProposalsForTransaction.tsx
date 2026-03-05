import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { loadTransactionProposals, type TransactionProposal } from "@/lib/consensus";

export function useProposalsForTransaction(proposalTxHash: Hex) {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<TransactionProposal[], Error>({
		queryKey: ["proposalsForTransactionHash", settings.consensus, proposalTxHash, settings.maxBlockRange],
		queryFn: () =>
			loadTransactionProposals({
				provider,
				consensus: settings.consensus,
				safeTxHash: proposalTxHash,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialData: [],
		refetchInterval: 10000,
	});
}
