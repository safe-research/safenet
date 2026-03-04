import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useProvider } from "@/hooks/useProvider";
import { useSettings } from "@/hooks/useSettings";
import { type AttestationStatus, loadLatestAttestationStatus } from "@/lib/coordinator/signing";

export function useAttestationStatus(safeTxHash: Hex, epoch: bigint, proposedAt: bigint, attestedAt: bigint | null) {
	const [settings] = useSettings();
	const provider = useProvider();
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
			loadLatestAttestationStatus({
				provider,
				consensus: settings.consensus,
				safeTxHash,
				epoch,
				proposedAt,
				attestedAt,
				maxBlockRange: BigInt(settings.maxBlockRange),
			}),
		initialData: null,
		refetchInterval: (query) => {
			return query.state.data?.completed ? false : 1000;
		},
	});
}
