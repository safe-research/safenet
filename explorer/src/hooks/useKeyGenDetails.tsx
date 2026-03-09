import { useQuery } from "@tanstack/react-query";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { loadEpochGroupId } from "@/lib/consensus";
import { type KeyGenStatus, loadKeyGenDetails } from "@/lib/coordinator/keygen";

export function useKeyGenDetails({
	epoch,
	endBlock,
	prevStagedAt,
	enabled = true,
}: {
	epoch: bigint;
	endBlock: bigint;
	prevStagedAt?: bigint;
	enabled?: boolean;
}) {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<KeyGenStatus | null, Error>({
		queryKey: ["keyGenDetails", settings.consensus, epoch.toString(), endBlock.toString()],
		queryFn: async () => {
			const gid = await loadEpochGroupId(provider, settings.consensus, epoch);
			return loadKeyGenDetails({
				provider,
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
