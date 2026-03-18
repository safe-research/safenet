import { TransactionListControls } from "@/components/transaction/TransactionListControls";
import { TransactionProposalsList } from "@/components/transaction/TransactionProposalsList";
import type { TransactionProposalWithStatus } from "@/lib/consensus";

export function RecentTransactionProposals({
	proposals,
	itemsToShow,
	onShowMore,
	isFetching,
	dataUpdatedAt,
	autoRefresh,
	onRefetch,
	onToggleAutoRefresh,
	isLoading,
}: {
	proposals: TransactionProposalWithStatus[];
	itemsToShow: number;
	onShowMore: () => void;
	isFetching: boolean;
	dataUpdatedAt: number;
	autoRefresh: boolean;
	onRefetch: () => void;
	onToggleAutoRefresh: () => void;
	isLoading?: boolean;
}) {
	return (
		<>
			<TransactionListControls
				isFetching={isFetching}
				dataUpdatedAt={dataUpdatedAt}
				autoRefresh={autoRefresh}
				onRefetch={onRefetch}
				onToggleAutoRefresh={onToggleAutoRefresh}
			/>
			<TransactionProposalsList
				proposals={proposals.slice(0, itemsToShow)}
				label={isLoading ? "" : `${proposals.length} recent proposals`}
				hasMore={proposals.length > itemsToShow}
				onShowMore={onShowMore}
				isLoading={isLoading}
				skeletonCount={itemsToShow}
				showMoreLabel="Show More"
			/>
		</>
	);
}
