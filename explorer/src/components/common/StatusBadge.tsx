import { Badge } from "@/components/common/Badge";
import type { ProposalStatus } from "@/lib/consensus";

export function StatusBadge({ status }: { status: ProposalStatus }) {
	switch (status) {
		case "TIMED_OUT":
			return <Badge variant="error">TIMED OUT</Badge>;
		case "ATTESTED":
			return <Badge variant="positive">ATTESTED</Badge>;
		case "DENIED":
			return <Badge variant="error">DENIED</Badge>;
		case "APPROVED":
			return <Badge variant="info">APPROVED</Badge>;
		default:
			return <Badge variant="pending">PROPOSED</Badge>;
	}
}
