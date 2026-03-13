import { type ChainInfo, SAFE_SERVICE_CHAINS } from "@/lib/chains";

export function useChainInfo(chainId: bigint): ChainInfo | undefined {
	return SAFE_SERVICE_CHAINS[chainId.toString()];
}
