import { Badge } from "@/components/common/Badge";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";
import { contrastColor } from "@/lib/utils";

const FALLBACK_COLOR = "#4B5563";

export function NetworkBadge({ chainId }: { chainId: bigint }) {
	const chain = SAFE_SERVICE_CHAINS[chainId.toString()];
	const label = (chain?.shortName ?? chainId.toString()).toUpperCase();
	const bgColor = chain?.color ?? FALLBACK_COLOR;
	return (
		<Badge bgColor={bgColor} fgColor={contrastColor(bgColor)}>
			{label}
		</Badge>
	);
}
