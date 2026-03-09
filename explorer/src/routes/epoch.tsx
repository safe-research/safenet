import { createFileRoute } from "@tanstack/react-router";
import { ConditionalBackButton } from "@/components/BackButton";
import { EpochCard } from "@/components/epoch/EpochCard";
import { EpochRolloverItem } from "@/components/epoch/EpochRolloverItem";
import { Container, ContainerSectionTitle, ContainerTitle } from "@/components/Groups";
import { Skeleton } from "@/components/Skeleton";
import { useEpochRolloverHistory } from "@/hooks/useEpochRolloverHistory";
import { useEpochsState } from "@/hooks/useEpochsState";
import { useSettings } from "@/hooks/useSettings";

export const Route = createFileRoute("/epoch")({
	component: EpochPage,
});

function EpochPage() {
	const epochsState = useEpochsState();
	const rolloverHistory = useEpochRolloverHistory();
	const [settings] = useSettings();

	return (
		<Container>
			<ConditionalBackButton />
			<ContainerTitle>Epoch Info</ContainerTitle>

			{epochsState.data === null && <Skeleton className="w-full h-32 bg-primary/10" />}

			{epochsState.data !== null && (
				<div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-8">
					<EpochCard label="Current Epoch" epoch={epochsState.data.active} groupId={epochsState.data.activeGroupId} />
					{epochsState.data.staged > 0n && epochsState.data.stagedGroupId !== null && (
						<EpochCard
							label="Staged Epoch"
							epoch={epochsState.data.staged}
							groupId={epochsState.data.stagedGroupId}
							rolloverBlock={epochsState.data.rolloverBlock}
						/>
					)}
				</div>
			)}

			<div className="mt-8 space-y-4">
				<ContainerSectionTitle>Epoch Rollover History</ContainerSectionTitle>

				{rolloverHistory.isLoading && <Skeleton className="w-full h-20 bg-primary/10" />}

				{rolloverHistory.entries.length === 0 && !rolloverHistory.isLoading && (
					<p className="text-sm text-muted">No epoch rollover events found in the current block range.</p>
				)}

				{rolloverHistory.entries.map((entry, index) => {
					const prevEntry = rolloverHistory.entries[index + 1];
					return (
						<EpochRolloverItem
							key={entry.proposedEpoch.toString()}
							entry={entry}
							prevStagedAt={prevEntry?.stagedAt}
							blocksPerEpoch={settings.blocksPerEpoch}
							consensus={settings.consensus}
						/>
					);
				})}

				{rolloverHistory.hasMore && (
					<button
						type="button"
						className="w-full py-2 text-sm text-primary hover:underline cursor-pointer disabled:opacity-50"
						onClick={rolloverHistory.loadMore}
						disabled={rolloverHistory.isFetching}
					>
						{rolloverHistory.isFetching ? "Loading…" : "Load more"}
					</button>
				)}
			</div>
		</Container>
	);
}
