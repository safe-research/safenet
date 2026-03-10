import { TRANSACTION_ROW_GRID_COLS, TransactionListRow } from "@/components/transaction/TransactionListRow";
import type { TransactionProposal } from "@/lib/consensus";

function TransactionListHeader() {
	return (
		<div
			className={`${TRANSACTION_ROW_GRID_COLS} px-3 py-1.5 text-[10px] font-medium text-sub-title uppercase tracking-wide`}
		>
			<div>Network</div>
			<div>Safe</div>
			<div>Tx Hash</div>
			<div>Details</div>
			<div className="text-right">When</div>
		</div>
	);
}

export function TransactionProposalsList({
	proposals,
	label,
	hasMore,
	onShowMore,
	isLoadingMore,
	showMoreLabel = "Show More",
}: {
	proposals: TransactionProposal[];
	label?: string;
	hasMore: boolean;
	onShowMore: () => void;
	isLoadingMore?: boolean;
	showMoreLabel?: string;
}) {
	return (
		<>
			{label !== undefined && <div className="w-full p-2 text-xs text-right">{label}</div>}
			<TransactionListHeader />
			<div className="space-y-2">
				{proposals.map((proposal) => (
					<div key={`${proposal.safeTxHash}:${proposal.epoch}`}>
						<TransactionListRow proposal={proposal} />
					</div>
				))}
				{hasMore && (
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
