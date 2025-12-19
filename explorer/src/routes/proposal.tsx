import { createFileRoute } from "@tanstack/react-router";
import z from "zod";
import { ConditionalBackButton } from "@/components/BackButton";
import { Box } from "@/components/Groups";
import { TransactionProposalDataDetails, TransactionProposalDetails } from "@/components/transaction/proposals";
import { useTransactionProposalDetails } from "@/hooks/useTransactionDetails";
import { bytes32Schema } from "@/lib/schemas";

const validateSearch = z.object({
	id: bytes32Schema,
});

export const Route = createFileRoute("/proposal")({
	validateSearch,
	component: Proposal,
});

export function Proposal() {
	const { id } = Route.useSearch();
	const details = useTransactionProposalDetails(id);
	const isAttested = details?.data?.attestedAt !== null;
	return (
		<div className="bg-gray-50 h-full max-h-full">
			<div className="max-w-4xl mx-auto px-4 py-12 sm:px-6 lg:px-8">
				<ConditionalBackButton />
				{details.isLoading && "Loading ..."}
				{details.data !== null && (
					<div className={"space-y-4"}>
						<Box>
							<TransactionProposalDetails proposal={details.data.proposal} hideProposedAt />
						</Box>
						<Box className={`${isAttested ? "border-green-400" : "border-yellow-400"}`}>
							<p className={"text-xs"}>Message: {details.data.proposal.message}</p>
							<p>Proposed at block {details.data.proposal.proposedAt}</p>
							{isAttested && <p>Attested at block {details.data.attestedAt}</p>}
							{!isAttested && <p>Attestetation pending</p>}
						</Box>
						<Box>
							<TransactionProposalDataDetails proposal={details.data.proposal} />
						</Box>
					</div>
				)}
				{!details.isFetching && details.data === null && <Box>"Could not load proposal!"</Box>}
			</div>
		</div>
	);
}
