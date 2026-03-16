import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useConsensusState } from "@/hooks/useConsensusState";
import { useSettings } from "@/hooks/useSettings";
import {
	getConsensusWorker,
	getProposalStatus,
	type LoadTransactionProposalsResult,
	type TransactionProposalWithStatus,
} from "@/lib/consensus";

export function useProposalsForTransaction(proposalTxHash: Hex) {
	const [settings] = useSettings();
	const consensusState = useConsensusState();
	const currentBlock = consensusState.data.currentBlock;

	return useQuery<LoadTransactionProposalsResult, Error, TransactionProposalWithStatus[]>({
		queryKey: ["proposalsForTransactionHash", settings.consensus, proposalTxHash, settings.maxBlockRange],
		queryFn: () =>
			getConsensusWorker().loadTransactionProposals({
				rpc: settings.rpc,
				consensus: settings.consensus,
				safeTxHash: proposalTxHash,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		select: (data) =>
			data.proposals.map((p) => ({ ...p, status: getProposalStatus(p, currentBlock, settings.signingTimeout) })),
		initialData: { proposals: [], fromBlock: 0n, toBlock: 0n },
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
	});
}
