import { useQuery } from "@tanstack/react-query";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { type EpochsState, loadEpochsState } from "@/lib/consensus";

export function useEpochsState() {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<EpochsState | null, Error>({
		queryKey: ["epochsState", settings.consensus],
		queryFn: () => loadEpochsState(provider, settings.consensus),
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
		initialData: null,
	});
}
