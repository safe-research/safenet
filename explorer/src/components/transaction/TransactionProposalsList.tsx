import {
	TransactionListRow,
	TransactionListRowSkeleton,
	TransactionRowGrid,
} from "@/components/transaction/TransactionListRow";
import type { TransactionProposal } from "@/lib/consensus";

const SKELETON_ROWS = ["skeleton-0", "skeleton-1", "skeleton-2"];

function TransactionListHeader() {
	return (
		<TransactionRowGrid className="px-3 py-1.5 text-2xs font-medium text-sub-title uppercase tracking-wide">
			<div>Network</div>
			<div>Safe</div>
			<div>Tx Hash</div>
			<div>Details</div>
			<div className="text-right">When</div>
		</TransactionRowGrid>
	);
}

export function TransactionProposalsList({
	proposals,
	label,
	hasMore,
	onShowMore,
	isLoading,
	isLoadingMore,
	showMoreLabel = "Show More",
}: {
	proposals: TransactionProposal[];
	label?: string;
	hasMore: boolean;
	onShowMore: () => void;
	isLoading?: boolean;
	isLoadingMore?: boolean;
	showMoreLabel?: string;
}) {
	return (
		<>
			{label !== undefined && <div className="w-full p-2 text-xs text-right">{label}</div>}
			<div className="hidden sm:block">
				<TransactionListHeader />
			</div>
			<div className="space-y-2">
				{isLoading ? (
					SKELETON_ROWS.map((key) => <TransactionListRowSkeleton key={key} />)
				) : proposals.length === 0 ? (
					<div className="w-full p-8 text-center text-sub-title">No recent proposals found</div>
				) : (
					proposals.map((proposal) => (
						<div key={`${proposal.safeTxHash}:${proposal.epoch}`}>
							<TransactionListRow proposal={proposal} />
						</div>
					))
				)}
				{!isLoading && hasMore && (
					<button
						type="button"
						className="w-full p-2 text-center cursor-pointer"
						onClick={onShowMore}
						disabled={isLoadingMore}
					>
						{isLoadingMore ? <output aria-label="Loading">…</output> : showMoreLabel}
					</button>
				)}
			</div>
		</>
	);
}
