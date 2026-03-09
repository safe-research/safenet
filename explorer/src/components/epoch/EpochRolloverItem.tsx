import { useState } from "react";
import { KeyGenStatusItem } from "@/components/epoch/KeyGenStatusItem";
import { Box } from "@/components/Groups";
import { Skeleton } from "@/components/Skeleton";
import { useKeyGenDetails } from "@/hooks/useKeyGenDetails";
import type { EpochRolloverEntry } from "@/lib/consensus";

export function EpochRolloverItem({ entry, prevStagedAt }: { entry: EpochRolloverEntry; prevStagedAt?: bigint }) {
	const [expanded, setExpanded] = useState(false);

	const keyGenDetails = useKeyGenDetails({
		gid: entry.groupId,
		endBlock: entry.stagedAt,
		prevStagedAt,
		enabled: expanded,
	});

	return (
		<Box className="space-y-3">
			<div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
				<span className="font-semibold text-title">
					Epoch {entry.activeEpoch.toString()} → {entry.proposedEpoch.toString()}
				</span>
				<div className="text-sm text-muted">
					<p>Staged: block {entry.stagedAt.toString()}</p>
				</div>
			</div>

			<button
				type="button"
				className="text-sm text-primary hover:underline cursor-pointer"
				onClick={() => setExpanded(!expanded)}
			>
				{expanded ? "Hide details ▲" : "Show details ▼"}
			</button>

			{expanded && (
				<div className="mt-2">
					{keyGenDetails.isFetching && keyGenDetails.data === null && (
						<Skeleton className="w-full h-10 bg-primary/10" />
					)}
					{keyGenDetails.data !== null && <KeyGenStatusItem status={keyGenDetails.data} />}
					{!keyGenDetails.isFetching && keyGenDetails.data === null && (
						<p className="text-sm text-muted">No key generation events found for this epoch.</p>
					)}
				</div>
			)}
		</Box>
	);
}
