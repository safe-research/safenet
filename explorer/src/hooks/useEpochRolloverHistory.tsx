import { useInfiniteQuery } from "@tanstack/react-query";
import { useCallback } from "react";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { loadEpochRolloverHistory } from "@/lib/consensus";

export function useEpochRolloverHistory() {
	const [settings] = useSettings();
	const provider = useProvider();
	const maxBlockRange = BigInt(settings.maxBlockRange);

	const query = useInfiniteQuery({
		queryKey: ["epochRolloverHistory", settings.consensus, settings.maxBlockRange],
		queryFn: async ({ pageParam }) => {
			return loadEpochRolloverHistory({
				provider,
				consensus: settings.consensus,
				maxBlockRange,
				cursor: pageParam,
			});
		},
		initialPageParam: undefined as bigint | undefined,
		getNextPageParam: (lastPage) => {
			if (lastPage.reachedGenesis) return undefined;
			const oldest = lastPage.entries.at(-1);
			// Fall back to fromBlock so an empty page still advances the cursor
			// backwards instead of stopping pagination prematurely.
			return oldest?.stagedAt ?? lastPage.fromBlock;
		},
		refetchInterval: settings.refetchInterval > 0 ? settings.refetchInterval : false,
	});

	const entries = query.data?.pages.flatMap((page) => page.entries) ?? [];

	const loadMore = useCallback(() => {
		if (query.hasNextPage && !query.isFetchingNextPage) {
			query.fetchNextPage();
		}
	}, [query.hasNextPage, query.isFetchingNextPage, query.fetchNextPage]);

	return {
		entries,
		loadMore,
		hasMore: query.hasNextPage,
		isLoading: query.isLoading,
		isFetching: query.isFetching,
	};
}
