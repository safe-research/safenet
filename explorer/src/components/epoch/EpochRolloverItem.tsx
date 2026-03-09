import { useState } from "react";
import type { Address, Hex } from "viem";
import { KeyGenStatusItem } from "@/components/epoch/KeyGenStatusItem";
import { Skeleton } from "@/components/Skeleton";
import { useEpochGroupId } from "@/hooks/useEpochGroupId";
import { useKeyGenDetails } from "@/hooks/useKeyGenDetails";
import type { EpochRolloverEntry } from "@/lib/consensus";

export function EpochRolloverItem({
	entry,
	prevStagedAt,
	blocksPerEpoch,
	consensus,
}: {
	entry: EpochRolloverEntry;
	prevStagedAt?: bigint;
	blocksPerEpoch?: number;
	consensus: Address;
}) {
	const [expanded, setExpanded] = useState(false);

	const startBlock = computeStartBlock(entry.proposedAt, prevStagedAt, blocksPerEpoch);
	const endBlock = entry.proposedAt;

	const gidQuery = useEpochGroupId(entry.proposedEpoch, consensus, expanded);
	const gid: Hex | null = gidQuery.data ?? null;

	const keyGenDetails = useKeyGenDetails({
		gid,
		startBlock,
		endBlock,
		enabled: expanded,
	});

	return (
		<div className="bg-surface-1 border border-surface-outline rounded-lg p-4 space-y-3">
			<div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
				<span className="font-semibold text-title">
					Epoch {entry.activeEpoch.toString()} → {entry.proposedEpoch.toString()}
				</span>
				<div className="text-sm text-muted space-y-0.5 sm:text-right">
					<p>Proposed: block {entry.proposedAt.toString()}</p>
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
					{!keyGenDetails.isFetching && keyGenDetails.data === null && gid !== null && (
						<p className="text-sm text-muted">No key generation events found for this epoch.</p>
					)}
				</div>
			)}
		</div>
	);
}

function computeStartBlock(proposedAt: bigint, prevStagedAt?: bigint, blocksPerEpoch?: number): bigint {
	if (blocksPerEpoch) {
		const bpe = BigInt(blocksPerEpoch);
		return proposedAt - (proposedAt % bpe);
	}
	if (prevStagedAt !== undefined) {
		return prevStagedAt;
	}
	return proposedAt > 10000n ? proposedAt - 10000n : 0n;
}
