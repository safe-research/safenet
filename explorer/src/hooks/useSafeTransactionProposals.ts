import { type InfiniteData, useInfiniteQuery } from "@tanstack/react-query";
import type { Address } from "viem";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import {
	type LoadTransactionProposalsResult,
	loadTransactionProposals,
	type TransactionProposal,
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
	const provider = useProvider();

	return useInfiniteQuery<
		LoadTransactionProposalsResult,
		Error,
		InfiniteData<TransactionProposal[]>,
		unknown[],
		bigint | undefined
	>({
		queryKey: ["safeProposals", safeAddress, chainId.toString(), settings.consensus, settings.maxBlockRange],
		refetchInterval: autoRefresh ? settings.refetchInterval : false,
		// pageParam is the toBlock for this window. undefined on the first page so
		// loadTransactionProposals resolves it from the current block at fetch time,
		// re-anchoring to the latest block on every refetch of page 0.
		queryFn: ({ pageParam: toBlock }) =>
			loadTransactionProposals({
				provider,
				consensus: settings.consensus,
				safe: safeAddress,
				toBlock,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialPageParam: undefined,
		// The next window ends one block before where the previous one started.
		getNextPageParam: (lastPage) => (lastPage.fromBlock > 0n ? lastPage.fromBlock - 1n : undefined),
		select: (data): InfiniteData<TransactionProposal[]> => ({
			pages: data.pages.map((p) => p.proposals),
			pageParams: data.pageParams,
		}),
	});
}
