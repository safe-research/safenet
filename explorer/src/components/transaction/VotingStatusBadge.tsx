import { Badge } from "@/components/common/Badge";
import type { VotingStatus } from "@/lib/oracle";

export function VotingStatusBadge({ status }: { status: VotingStatus }) {
	if (status == null) return null;

	if (status.kind === "generic") {
		return <Badge variant={status.approved ? "positive" : "error"}>{status.approved ? "APPROVED" : "DENIED"}</Badge>;
	}

	switch (status.state) {
		case "RESOLVED_APPROVED":
			return <Badge variant="positive">APPROVED</Badge>;
		case "RESOLVED_DENIED":
			return <Badge variant="error">DENIED</Badge>;
		case "TIMED_OUT":
			return <Badge variant="error">TIMED OUT</Badge>;
		case "FROZEN":
			return <Badge variant="pending">ARBITRATING</Badge>;
		default:
			return <Badge variant="pending">PENDING</Badge>;
	}
}
