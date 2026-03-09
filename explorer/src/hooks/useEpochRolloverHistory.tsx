import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { type EpochRolloverEntry, loadEpochRolloverHistory } from "@/lib/consensus";

export function useEpochRolloverHistory() {
	const [settings] = useSettings();
	const provider = useProvider();
	const maxBlockRange = BigInt(settings.maxBlockRange);

	const [allEntries, setAllEntries] = useState<EpochRolloverEntry[]>([]);
	const [cursor, setCursor] = useState<bigint | undefined>(undefined);
	const [hasMore, setHasMore] = useState(true);

	const query = useQuery({
		queryKey: ["epochRolloverHistory", settings.consensus, settings.maxBlockRange, cursor?.toString()],
		queryFn: async () => {
			const result = await loadEpochRolloverHistory({
				provider,
				consensus: settings.consensus,
				maxBlockRange,
				cursor,
			});

			if (result.reachedGenesis) {
				setHasMore(false);
			}

			setAllEntries((prev) => [...prev, ...result.entries]);

			return result.entries;
		},
		refetchInterval: () => {
			if (cursor !== undefined) return false;
			return settings.refetchInterval > 0 ? settings.refetchInterval : false;
		},
	});

	const loadMore = useCallback(() => {
		if (!hasMore) return;
		const oldest = allEntries.at(-1);
		if (oldest) {
			setCursor(oldest.stagedAt);
		}
	}, [allEntries, hasMore]);

	return {
		entries: allEntries,
		loadMore,
		hasMore,
		isLoading: query.isLoading,
		isFetching: query.isFetching,
	};
}
