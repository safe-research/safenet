import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { type Block, createPublicClient, http, type PublicClient } from "viem";
import { useSettings } from "@/hooks/useSettings";

export function useProvider(): PublicClient {
	const [settings] = useSettings();
	const provider = useMemo(() => {
		return createPublicClient({
			transport: http(settings.rpc),
		});
	}, [settings.rpc]);
	return provider;
}

export function useChainId() {
	const provider = useProvider();
	return useQuery<bigint | null, Error>({
		queryKey: ["chainId"],
		queryFn: () => provider.getChainId().then((id) => BigInt(id)),
		refetchInterval: 10000,
		initialData: null,
	});
}

export function useBlockInfo(blockNumber: bigint) {
	const provider = useProvider();
	return useQuery<Block | null, Error>({
		queryKey: ["blockInfo", blockNumber.toString()],
		queryFn: () => provider.getBlock({ blockNumber }),
		refetchInterval: 10000,
		initialData: null,
	});
}
