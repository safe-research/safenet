import type { Hex } from "viem";
import { Box, BoxTitle } from "@/components/Groups";

export function EpochCard({
	label,
	epoch,
	groupId,
	rolloverBlock,
}: {
	label: string;
	epoch: bigint;
	groupId: Hex;
	rolloverBlock?: bigint;
}) {
	return (
		<Box>
			<BoxTitle>{label}</BoxTitle>
			<dl className="space-y-1 text-sm">
				<div className="flex justify-between">
					<dt className="text-muted">Epoch</dt>
					<dd className="font-mono">{epoch.toString()}</dd>
				</div>
				<div className="flex justify-between">
					<dt className="text-muted">Group ID</dt>
					<dd className="font-mono">{groupId.slice(0, 18)}…</dd>
				</div>
				{rolloverBlock !== undefined && rolloverBlock > 0n && (
					<div className="flex justify-between">
						<dt className="text-muted">Rollover Block</dt>
						<dd className="font-mono">{rolloverBlock.toString()}</dd>
					</div>
				)}
			</dl>
		</Box>
	);
}
