import { useQuery } from "@tanstack/react-query";
import { zeroHash } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { type ConsensusState, getConsensusWorker } from "@/lib/consensus";

export function useConsensusState() {
	const [settings] = useSettings();
	return useQuery<ConsensusState, Error>({
		queryKey: ["consensusState", settings.consensus],
		queryFn: () => getConsensusWorker().loadConsensusState({ rpc: settings.rpc, consensus: settings.consensus }),
		refetchInterval: () => (settings.refetchInterval > 0 ? settings.refetchInterval : false),
		initialData: { currentBlock: 0n, currentEpoch: 0n, currentGroupId: zeroHash, chainId: 0n },
	});
}
