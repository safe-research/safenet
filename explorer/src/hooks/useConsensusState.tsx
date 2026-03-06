import { useQuery } from "@tanstack/react-query";
import { zeroHash } from "viem";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { type ConsensusState, loadConsensusState } from "@/lib/consensus";

export function useConsensusState() {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<ConsensusState, Error>({
		queryKey: ["consensusState", settings.consensus],
		queryFn: () => loadConsensusState(provider, settings.consensus),
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
		initialData: { currentBlock: 0n, currentEpoch: 0n, currentGroupId: zeroHash },
	});
}
