import { Badge } from "@/components/common/Badge";

export function StatusBadge({ attested }: { attested: boolean }) {
	if (attested) {
		return <Badge className="border-positive text-positive">ATTESTED</Badge>;
	}
	return <Badge className="border-pending text-pending">PROPOSED</Badge>;
}
