import { useQuery } from "@tanstack/react-query";
import type { Address } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { loadSentinelInfoMap, type SentinelInfo } from "@/lib/sentinels/info";

export function useSentinelInfoMap() {
	const [settings] = useSettings();
	return useQuery<Map<Address, SentinelInfo> | null, Error>({
		queryKey: ["sentinelInfoMap", settings.sentinelInfo],
		queryFn: () => loadSentinelInfoMap(settings.sentinelInfo),
		initialData: null,
	});
}
