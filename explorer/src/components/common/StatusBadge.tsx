import { Badge } from "@/components/common/Badge";
import type { ProposalStatus } from "@/lib/consensus";

export function StatusBadge({ status }: { status: ProposalStatus }) {
	if (status === "TIMED_OUT") {
		return <Badge variant="error">TIMED OUT</Badge>;
	}
	if (status === "ATTESTED") {
		return <Badge variant="positive">ATTESTED</Badge>;
	}
	return <Badge variant="pending">PROPOSED</Badge>;
}
