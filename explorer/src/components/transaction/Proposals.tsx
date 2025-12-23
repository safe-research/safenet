import { Link } from "@tanstack/react-router";
import { useMemo } from "react";
import type { TransactionProposal } from "@/lib/consensus";
import { calculateSafeTxHash } from "@/lib/safe/hashing";
import { Box } from "../Groups";
import { SafeTxOverview } from "./SafeTxOverview";

export const TransactionProposalDetails = ({
	proposal,
	hideProposedAt,
	disableLinks,
}: {
	proposal: TransactionProposal;
	hideProposedAt?: boolean;
	disableLinks?: boolean;
}) => {
	const safeTxHash = useMemo(() => {
		return calculateSafeTxHash(proposal.transaction);
	}, [proposal]);
	const timestamp = hideProposedAt ? undefined : proposal.proposedAt.toString();
	return (
		<SafeTxOverview
			transaction={proposal.transaction}
			title={`Safe Tx Hash: ${safeTxHash}`}
			timestamp={timestamp}
			disableLinks={disableLinks}
		/>
	);
};

export const TransactionProposalList = ({
	proposals,
	onShowMore,
	itemsToDisplay,
}: {
	proposals: TransactionProposal[];
	onShowMore?: () => void;
	itemsToDisplay: number;
}) => {
	return (
		<>
			<div className="w-full p-2 text-xs text-right">{proposals.length} recent proposals</div>
			<div className={"space-y-4"}>
				{proposals.slice(0, itemsToDisplay).map((proposal) => (
					<div key={proposal.message}>
						<Link to="/proposal" search={{ id: proposal.message }}>
							<Box className={"hover:bg-surface-hover"}>
								<TransactionProposalDetails proposal={proposal} disableLinks />
							</Box>
						</Link>
					</div>
				))}
				{onShowMore && proposals.length > itemsToDisplay && (
					<button type="button" className="w-full p-2 text-center cursor-pointer" onClick={onShowMore}>
						Show More
					</button>
				)}
			</div>
		</>
	);
};
