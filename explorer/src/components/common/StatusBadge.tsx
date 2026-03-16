import { Badge } from "@/components/common/Badge";

export type ProposalStatus = "ATTESTED" | "PROPOSED" | "TIMED_OUT";

export function StatusBadge({ status }: { status: ProposalStatus }) {
	if (status === "TIMED_OUT") {
		return <Badge className="bg-error text-white">TIMED OUT</Badge>;
	}
	if (status === "ATTESTED") {
		return <Badge className="bg-positive text-positive-foreground">ATTESTED</Badge>;
	}
	return <Badge className="bg-pending text-pending-foreground">PROPOSED</Badge>;
}
