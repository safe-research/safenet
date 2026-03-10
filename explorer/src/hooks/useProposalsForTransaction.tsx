import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import {
	type LoadTransactionProposalsResult,
	loadTransactionProposals,
	type TransactionProposal,
} from "@/lib/consensus";

export function useProposalsForTransaction(proposalTxHash: Hex) {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<LoadTransactionProposalsResult, Error, TransactionProposal[]>({
		queryKey: ["proposalsForTransactionHash", settings.consensus, proposalTxHash, settings.maxBlockRange],
		queryFn: () =>
			loadTransactionProposals({
				provider,
				consensus: settings.consensus,
				safeTxHash: proposalTxHash,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		select: (data) => data.proposals,
		initialData: { proposals: [], fromBlock: 0n, toBlock: 0n },
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
	});
}
