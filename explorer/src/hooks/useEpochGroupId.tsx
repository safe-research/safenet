import { useQuery } from "@tanstack/react-query";
import type { Address, Hex } from "viem";
import { useProvider } from "@/hooks/useProvider";
import { loadEpochGroupId } from "@/lib/consensus";

export function useEpochGroupId(epoch: bigint, consensus: Address, enabled = true) {
	const provider = useProvider();
	return useQuery<Hex, Error>({
		queryKey: ["epochGroupId", consensus, epoch.toString()],
		queryFn: () => loadEpochGroupId(provider, consensus, epoch),
		enabled,
	});
}
