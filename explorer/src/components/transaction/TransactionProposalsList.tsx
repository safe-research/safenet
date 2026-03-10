import { TransactionListRow } from "@/components/transaction/TransactionListRow";
import type { TransactionProposal } from "@/lib/consensus";

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
