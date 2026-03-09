import { TransactionProposalsList } from "@/components/transaction/TransactionProposalsList";
import type { TransactionProposal } from "@/lib/consensus";

export function RecentTransactionProposals({
	proposals,
	itemsToShow,
	onShowMore,
}: {
	proposals: TransactionProposal[];
	itemsToShow: number;
	onShowMore: () => void;
}) {
	return (
		<TransactionProposalsList
			proposals={proposals.slice(0, itemsToShow)}
			label={`${proposals.length} recent proposals`}
			hasMore={proposals.length > itemsToShow}
			onShowMore={onShowMore}
			showMoreLabel="Show More"
		/>
	);
}
