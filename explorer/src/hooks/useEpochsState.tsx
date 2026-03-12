import { useQuery } from "@tanstack/react-query";
import { useSettings } from "@/hooks/useSettings";
import { type EpochsState, getConsensusWorker } from "@/lib/consensus";

export function useEpochsState() {
	const [settings] = useSettings();
	return useQuery<EpochsState | null, Error>({
		queryKey: ["epochsState", settings.consensus],
		queryFn: () => getConsensusWorker().loadEpochsState({ rpc: settings.rpc, consensus: settings.consensus }),
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
		initialData: null,
	});
}
