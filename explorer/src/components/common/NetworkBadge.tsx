import { Badge } from "@/components/common/Badge";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";

export function NetworkBadge({ chainId }: { chainId: bigint }) {
	const chain = SAFE_SERVICE_CHAINS[chainId.toString()];
	const label = (chain?.shortName ?? chainId.toString()).toUpperCase();
	return <Badge className="border-surface-outline bg-surface-0 text-sub-title">{label}</Badge>;
}
