import { Link } from "@tanstack/react-router";
import { Box } from "@/components/Groups";
import { SafeTxOverview } from "@/components/transaction/SafeTxOverview";
import type { TransactionProposal } from "@/lib/consensus";

export function TransactionProposalItem({ proposal }: { proposal: TransactionProposal }) {
	return (
		<Link to="/safeTx" search={{ chainId: `${proposal.transaction.chainId}`, safeTxHash: proposal.safeTxHash }}>
			<Box className={`hover:bg-surface-hover ${proposal.attestedAt ? "border-positive" : "border-pending"}`}>
				<SafeTxOverview
					transaction={proposal.transaction}
					title={`Safe Tx Hash: ${proposal.safeTxHash}`}
					timestamp={`${proposal.proposedAt.block}`}
					disableLinks={true}
				/>
			</Box>
		</Link>
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
			<div className="space-y-4">
				{proposals.map((proposal) => (
					<div key={`${proposal.safeTxHash}:${proposal.epoch}`}>
						<TransactionProposalItem proposal={proposal} />
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
