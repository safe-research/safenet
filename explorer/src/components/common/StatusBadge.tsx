import { Badge } from "@/components/common/Badge";

export function StatusBadge({ attested }: { attested: boolean }) {
	if (attested) {
		return <Badge className="bg-positive text-[#000000]">ATTESTED</Badge>;
	}
	return <Badge className="bg-pending text-[#000000]">PROPOSED</Badge>;
}
