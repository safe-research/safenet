import { Badge } from "@/components/common/Badge";

export function StatusBadge({ attested }: { attested: boolean }) {
	if (attested) {
		return <Badge className="bg-positive text-positive-foreground">ATTESTED</Badge>;
	}
	return <Badge className="bg-pending text-pending-foreground">PROPOSED</Badge>;
}
