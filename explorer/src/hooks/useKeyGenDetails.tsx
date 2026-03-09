import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { type KeyGenStatus, loadKeyGenDetails } from "@/lib/coordinator/keygen";

export function useKeyGenDetails({
	gid,
	startBlock,
	endBlock,
	enabled = true,
}: {
	gid: Hex | null;
	startBlock: bigint;
	endBlock: bigint;
	enabled?: boolean;
}) {
	const [settings] = useSettings();
	const provider = useProvider();
	return useQuery<KeyGenStatus | null, Error>({
		queryKey: ["keyGenDetails", settings.consensus, gid, startBlock.toString(), endBlock.toString()],
		queryFn: () => {
			if (gid === null) throw new Error("gid is required");
			return loadKeyGenDetails({
				provider,
				consensus: settings.consensus,
				gid,
				startBlock,
				endBlock,
			});
		},
		enabled: enabled && gid !== null,
		refetchInterval: (query) => {
			const data = query.state.data;
			if (data?.finalized || data?.compromised) return false;
			return settings.refetchInterval > 0 ? settings.refetchInterval : false;
		},
		initialData: null,
	});
}
