import { Badge } from "@/components/common/Badge";
import type { ProposalStatus } from "@/lib/consensus";

export function StatusBadge({ status }: { status: ProposalStatus }) {
	switch (status) {
		case "TIMED_OUT":
			return <Badge variant="error">TIMED OUT</Badge>;
		case "ATTESTED":
			return <Badge variant="positive">ATTESTED</Badge>;
		default:
			return <Badge variant="pending">PROPOSED</Badge>;
	}
}
