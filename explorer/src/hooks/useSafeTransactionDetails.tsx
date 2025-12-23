import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { loadSafeTransactionDetails, type SafeTransaction } from "@/lib/safe/service";

export function useSafeTransactionDetails(chainId: bigint, safeTxHash: Hex) {
	return useQuery<SafeTransaction | null, Error>({
		queryKey: ["safeTxDetails", chainId.toString(), safeTxHash],
		queryFn: () => loadSafeTransactionDetails(chainId, safeTxHash),
		initialData: null,
	});
}
