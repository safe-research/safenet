import { type InfiniteData, useInfiniteQuery } from "@tanstack/react-query";
import type { Address } from "viem";
import { useConsensusState } from "@/hooks/useConsensusState";
import { useSettings } from "@/hooks/useSettings";
import {
	getConsensusWorker,
	getProposalStatus,
	type LoadTransactionProposalsResult,
	type TransactionProposalWithStatus,
} from "@/lib/consensus";

export function useSafeTransactionProposals({
	safeAddress,
	chainId,
	autoRefresh = false,
}: {
	safeAddress: Address;
	chainId: bigint;
	autoRefresh?: boolean;
}) {
	const [settings] = useSettings();
	const consensusState = useConsensusState();
	const currentBlock = consensusState.data.currentBlock;

	return useInfiniteQuery<
		LoadTransactionProposalsResult,
		Error,
		InfiniteData<TransactionProposalWithStatus[]>,
		unknown[],
		bigint | undefined
	>({
		queryKey: ["safeProposals", safeAddress, chainId.toString(), settings.consensus, settings.maxBlockRange],
		refetchInterval: autoRefresh ? settings.refetchInterval : false,
		// pageParam is the toBlock for this window. undefined on the first page so
		// loadTransactionProposals resolves it from the current block at fetch time,
		// re-anchoring to the latest block on every refetch of page 0.
		queryFn: ({ pageParam: toBlock }) =>
			getConsensusWorker().loadTransactionProposals({
				rpc: settings.rpc,
				consensus: settings.consensus,
				safe: safeAddress,
				toBlock,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialPageParam: undefined,
		// The next window ends one block before where the previous one started.
		getNextPageParam: (lastPage) => (lastPage.fromBlock > 0n ? lastPage.fromBlock - 1n : undefined),
		select: (data): InfiniteData<TransactionProposalWithStatus[]> => ({
			pages: data.pages.map((p) =>
				p.proposals.map((proposal) => ({
					...proposal,
					status: getProposalStatus(proposal, currentBlock, settings.signingTimeout),
				})),
			),
			pageParams: data.pageParams,
		}),
	});
}
