import { Badge } from "@/components/common/Badge";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import { contrastColor } from "@/lib/utils";

export function NetworkBadge({ chainId, title, className }: { chainId: bigint; title?: string; className?: string }) {
	const chain = SAFE_SERVICE_CHAINS[chainId.toString()];
	const label = (chain?.shortName ?? chainId.toString()).toUpperCase();
	const badgeProps = chain?.color
		? { bgColor: chain.color, fgColor: contrastColor(chain.color) }
		: { variant: "neutral" as const };
	return (
		<Badge {...badgeProps} title={title} className={className}>
			{label}
		</Badge>
	);
}
