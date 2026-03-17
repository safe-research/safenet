import { Badge } from "@/components/common/Badge";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import { contrastColor } from "@/lib/utils";

export function NetworkBadge({ chainId, title, className }: { chainId: bigint; title?: string; className?: string }) {
	const chain = SAFE_SERVICE_CHAINS[chainId.toString()];
	const label = (chain?.shortName ?? chainId.toString()).toUpperCase();
	if (chain?.color !== undefined) {
		return (
			<Badge bgColor={chain.color} fgColor={contrastColor(chain.color)} title={title} className={className}>
				{label}
			</Badge>
		);
	}
	return (
		<Badge variant="neutral" title={title} className={className}>
			{label}
		</Badge>
	);
}
