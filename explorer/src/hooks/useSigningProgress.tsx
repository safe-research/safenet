import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { type AttestationStatus, getCoordinatorWorker } from "@/lib/coordinator";

export function useAttestationStatus(safeTxHash: Hex, epoch: bigint, proposedAt: bigint, attestedAt: bigint | null) {
	const [settings] = useSettings();
	return useQuery<AttestationStatus | null, Error>({
		queryKey: [
			"signingStatusByTxHash",
			settings.consensus,
			safeTxHash,
			epoch.toString(),
			proposedAt.toString(),
			attestedAt?.toString(),
			settings.maxBlockRange,
		],
		queryFn: () =>
			getCoordinatorWorker().loadLatestAttestationStatus({
				rpc: settings.rpc,
				consensus: settings.consensus,
				safeTxHash,
				epoch,
				proposedAt,
				attestedAt,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialData: null,
		refetchInterval: (query) => {
			return query.state.data?.status !== "completed" && settings.refetchInterval > 0
				? settings.refetchInterval
				: false;
		},
	});
}
