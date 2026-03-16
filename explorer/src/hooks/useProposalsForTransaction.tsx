import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import {
	getConsensusWorker,
	type LoadTransactionProposalsResult,
	type TransactionProposalWithStatus,
} from "@/lib/consensus";

export function useProposalsForTransaction(proposalTxHash: Hex) {
	const [settings] = useSettings();

	return useQuery<LoadTransactionProposalsResult, Error, TransactionProposalWithStatus[]>({
		queryKey: ["proposalsForTransactionHash", settings.consensus, proposalTxHash, settings.maxBlockRange],
		queryFn: () =>
			getConsensusWorker().loadTransactionProposals({
				rpc: settings.rpc,
				consensus: settings.consensus,
				safeTxHash: proposalTxHash,
				maxBlockRange: BigInt(settings.maxBlockRange),
				signingTimeout: settings.signingTimeout,
			}),
		select: (data) => data.proposals,
		initialData: { proposals: [], fromBlock: 0n, toBlock: 0n },
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
	});
}
