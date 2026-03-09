import { type InfiniteData, useInfiniteQuery, useQuery } from "@tanstack/react-query";
import type { Address } from "viem";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { loadTransactionProposals, type TransactionProposal } from "@/lib/consensus";

export function useSafeTransactionProposals({ safeAddress, chainId }: { safeAddress: Address; chainId: bigint }) {
	const [settings] = useSettings();
	const provider = useProvider();

	// Capture the initial block once at mount to keep pagination boundaries stable across re-fetches.
	const { data: initialFromBlock } = useQuery({
		queryKey: ["safeProposalsInitialBlock", safeAddress, chainId.toString(), settings.consensus],
		queryFn: async () => {
			const blockNumber = await provider.getBlockNumber();
			return blockNumber > BigInt(settings.maxBlockRange) ? blockNumber - BigInt(settings.maxBlockRange) : 0n;
		},
		staleTime: Number.POSITIVE_INFINITY,
	});

	return useInfiniteQuery<TransactionProposal[], Error, InfiniteData<TransactionProposal[]>, unknown[], bigint>({
		queryKey: ["safeProposals", safeAddress, chainId.toString(), settings.consensus, settings.maxBlockRange],
		queryFn: ({ pageParam: fromBlock }) =>
			loadTransactionProposals({
				provider,
				consensus: settings.consensus,
				safe: safeAddress,
				fromBlock,
				toBlock: fromBlock + BigInt(settings.maxBlockRange) - 1n,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialPageParam: initialFromBlock ?? 0n,
		getNextPageParam: (_lastPage, _pages, lastFromBlock) => {
			const nextFrom = lastFromBlock - BigInt(settings.maxBlockRange);
			return nextFrom >= 0n ? nextFrom : undefined;
		},
		enabled: initialFromBlock !== undefined,
	});
}
