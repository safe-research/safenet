import { type InfiniteData, useInfiniteQuery } from "@tanstack/react-query";
import type { Address } from "viem";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { loadTransactionProposals, type TransactionProposal } from "@/lib/consensus";
import { getFromBlock } from "@/lib/utils";

// Each page carries the proposals it fetched alongside the fromBlock used, so
// getNextPageParam can derive the next window without a separate anchor query.
type PageData = { proposals: TransactionProposal[]; fromBlock: bigint };

export function useSafeTransactionProposals({ safeAddress, chainId }: { safeAddress: Address; chainId: bigint }) {
	const [settings] = useSettings();
	const provider = useProvider();

	return useInfiniteQuery<PageData, Error, InfiniteData<TransactionProposal[]>, unknown[], bigint | undefined>({
		queryKey: ["safeProposals", safeAddress, chainId.toString(), settings.consensus, settings.maxBlockRange],
		queryFn: async ({ pageParam }) => {
			// undefined means "first page" — anchor to the current block at fetch time.
			// On refetch, page 0 always receives undefined again, naturally re-anchoring to the latest block.
			const fromBlock = pageParam ?? (await getFromBlock(provider, BigInt(settings.maxBlockRange)));
			const proposals = await loadTransactionProposals({
				provider,
				consensus: settings.consensus,
				safe: safeAddress,
				fromBlock,
				toBlock: fromBlock + BigInt(settings.maxBlockRange) - 1n,
				maxBlockRange: BigInt(settings.maxBlockRange),
			});
			return { proposals, fromBlock };
		},
		initialPageParam: undefined,
		getNextPageParam: (lastPage) => {
			const nextFrom = lastPage.fromBlock - BigInt(settings.maxBlockRange);
			return nextFrom >= 0n ? nextFrom : undefined;
		},
		select: (data): InfiniteData<TransactionProposal[]> => ({
			pages: data.pages.map((p) => p.proposals),
			pageParams: data.pageParams,
		}),
	});
}
