import { SAFE_SERVICE_CHAINS } from "@/lib/chains";

export function NetworkBadge({ chainId }: { chainId: bigint }) {
	const chain = SAFE_SERVICE_CHAINS[chainId.toString()];
	const label = (chain?.shortName ?? chainId.toString()).toUpperCase();
	return (
		<span className="inline-block px-1.5 py-0.5 text-[10px] font-semibold leading-none rounded border border-surface-outline bg-surface-0 text-sub-title">
			{label}
		</span>
	);
}
