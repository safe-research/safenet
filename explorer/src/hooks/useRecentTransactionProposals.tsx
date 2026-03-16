import { useQuery } from "@tanstack/react-query";
import { useConsensusState } from "@/hooks/useConsensusState";
import { useSettings } from "@/hooks/useSettings";
import {
	getConsensusWorker,
	getProposalStatus,
	type LoadTransactionProposalsResult,
	type TransactionProposalWithStatus,
} from "@/lib/consensus";

export function useRecentTransactionProposals(autoRefresh = true) {
	const [settings] = useSettings();
	const consensusState = useConsensusState();
	const currentBlock = consensusState.data.currentBlock;

	return useQuery<LoadTransactionProposalsResult, Error, TransactionProposalWithStatus[]>({
		queryKey: ["recentProposals", settings.consensus, settings.maxBlockRange],
		queryFn: () =>
			getConsensusWorker().loadTransactionProposals({
				rpc: settings.rpc,
				consensus: settings.consensus,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		select: (data) =>
			data.proposals.map((p) => ({ ...p, status: getProposalStatus(p, currentBlock, settings.signingTimeout) })),
		refetchInterval: () => (autoRefresh && settings.refetchInterval > 0 ? settings.refetchInterval : false),
		initialData: { proposals: [], fromBlock: 0n, toBlock: 0n },
	});
}
