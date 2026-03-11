import { useQuery } from "@tanstack/react-query";
import type { Hex } from "viem";
import { useSettings } from "@/hooks/useSettings";
import { getConsensusWorker, type SafeTransaction } from "@/lib/consensus";
import { loadSafeTransactionDetails } from "@/lib/safe/service";

const findAny = async (...promises: Promise<SafeTransaction | null>[]): Promise<SafeTransaction | null> => {
	try {
		// `Promise.any` will resolve with the first resolved value, skipping any rejections. It only rejects if there
		// are no resolved values. We use this property to "get the first `SafeTransaction` we can find from a list of
		// promises", by throwing on "not found".
		const result = await Promise.any(
			promises.map((promise) =>
				promise.then((data) => {
					if (data === null) {
						throw new Error("not found");
					}
					return data;
				}),
			),
		);
		return result;
	} catch {
		return null;
	}
};

export function useSafeTransactionDetails(chainId: bigint, safeTxHash: Hex) {
	const [settings] = useSettings();
	return useQuery<SafeTransaction | null, Error>({
		queryKey: ["safeTxDetails", chainId.toString(), safeTxHash, settings.consensus, settings.maxBlockRange],
		queryFn: () =>
			findAny(
				loadSafeTransactionDetails(chainId, safeTxHash),
				getConsensusWorker().loadProposedSafeTransaction({
					rpc: settings.rpc,
					consensus: settings.consensus,
					safeTxHash,
					maxBlockRange: BigInt(settings.maxBlockRange),
				}),
			),
		initialData: null,
	});
}
