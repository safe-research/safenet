import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getCoordinatorWorker, type KeyGenStatus } from "@/lib/coordinator";

export function useKeyGenDetails({
	gid,
	endBlock,
	prevStagedAt,
	enabled = true,
}: {
	gid: Hex;
	endBlock: bigint;
	prevStagedAt?: bigint;
	enabled?: boolean;
}) {
	const [settings] = useSettings();
	return useQuery<KeyGenStatus | null, Error>({
		queryKey: ["keyGenDetails", settings.consensus, gid, endBlock.toString()],
		queryFn: async () => {
			return getCoordinatorWorker().loadKeyGenDetails({
				rpc: settings.rpc,
				consensus: settings.consensus,
				gid,
				endBlock,
				blocksPerEpoch: settings.blocksPerEpoch,
				prevStagedAt,
				maxBlockRange: BigInt(settings.maxBlockRange),
			});
		},
		enabled,
		refetchInterval: (query) => {
			const data = query.state.data;
			if (data?.finalized || data?.compromised) return false;
			return settings.refetchInterval > 0 ? settings.refetchInterval : false;
		},
		initialData: null,
	});
}
