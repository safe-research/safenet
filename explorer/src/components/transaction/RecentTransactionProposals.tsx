import { Link } from "@tanstack/react-router";
import { Box } from "@/components/Groups";
import { SafeTxOverview } from "@/components/transaction/SafeTxOverview";
import type { TransactionProposal } from "@/lib/consensus";

function RecentTransactionProposal({ proposal }: { proposal: TransactionProposal }) {
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
		<>
			<div className="w-full p-2 text-xs text-right">{proposals.length} recent proposals</div>
			<div className={"space-y-4"}>
				{proposals.slice(0, itemsToShow).map((proposal) => (
					<div key={`${proposal.safeTxHash}:${proposal.epoch}`}>
						<RecentTransactionProposal proposal={proposal} />
					</div>
				))}
				{proposals.length > itemsToShow && (
					<button type="button" className="w-full p-2 text-center cursor-pointer" onClick={onShowMore}>
						Show More
					</button>
				)}
			</div>
		</>
	);
}
