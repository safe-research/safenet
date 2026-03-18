import { Spinner } from "@/components/common/Spinner";
import {
	TransactionListRow,
	TransactionListRowSkeleton,
	TransactionRowGrid,
} from "@/components/transaction/TransactionListRow";
import type { TransactionProposalWithStatus } from "@/lib/consensus";

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
	skeletonCount = 1,
	isLoadingMore,
	showMoreLabel = "Show More",
	emptyLabel = "No transactions found",
}: {
	proposals: TransactionProposalWithStatus[];
	label?: string;
	hasMore: boolean;
	onShowMore: () => void;
	isLoading?: boolean;
	skeletonCount?: number;
	isLoadingMore?: boolean;
	showMoreLabel?: string;
	emptyLabel?: string;
}) {
	return (
		<>
			{(label !== undefined || isLoading) && (
				<div className="w-full p-2 text-xs text-right flex justify-end items-center">
					{isLoading ? <Spinner className="h-3 w-3" /> : label}
					&nbsp;recent proposals
				</div>
			)}
			<div className="hidden sm:block">
				<TransactionListHeader />
			</div>
			<div className="space-y-2">
				{isLoading ? (
					// biome-ignore lint/suspicious/noArrayIndexKey: skeleton rows are static placeholders with no identity
					Array.from({ length: skeletonCount }, (_, i) => <TransactionListRowSkeleton key={`skeleton-${i}`} />)
				) : proposals.length === 0 ? (
					<div className="w-full p-8 text-center text-sub-title">{emptyLabel}</div>
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
