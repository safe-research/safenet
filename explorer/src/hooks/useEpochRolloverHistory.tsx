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
	const [cursor, setCursor] = useState<bigint | null>(null);
	const [hasMore, setHasMore] = useState(true);

	const query = useQuery<EpochRolloverEntry[], Error>({
		queryKey: ["epochRolloverHistory", settings.consensus, settings.maxBlockRange, cursor?.toString()],
		queryFn: async () => {
			const currentBlock = await provider.getBlockNumber();
			const toBlock = cursor ?? currentBlock;
			const fromBlock = toBlock > maxBlockRange ? toBlock - maxBlockRange : 0n;

			const entries = await loadEpochRolloverHistory({
				provider,
				consensus: settings.consensus,
				fromBlock,
				toBlock: cursor === null ? "latest" : toBlock,
			});

			if (fromBlock === 0n) {
				setHasMore(false);
			}

			setAllEntries((prev) => {
				const existingKeys = new Set(prev.map((e) => `${e.proposedEpoch}`));
				const newEntries = entries.filter((e) => !existingKeys.has(`${e.proposedEpoch}`));
				return [...prev, ...newEntries].sort((a, b) => (a.proposedAt < b.proposedAt ? 1 : -1));
			});

			return entries;
		},
		refetchInterval: () => {
			// Only auto-refetch the initial load (no cursor), not "load more" fetches
			if (cursor !== null) return false;
			return settings.refetchInterval > 0 ? settings.refetchInterval : false;
		},
	});

	const loadMore = useCallback(() => {
		if (!hasMore) return;
		const oldest = allEntries.at(-1);
		if (oldest) {
			setCursor(oldest.proposedAt);
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
