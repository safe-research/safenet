import { createFileRoute } from "@tanstack/react-router";
import z from "zod";
import { ConditionalBackButton } from "@/components/BackButton";
import { Box, Container, ContainerTitle } from "@/components/Groups";
import { TransactionProposalDetails } from "@/components/transaction/Proposals";
import { TransactionDataDetails } from "@/components/transaction/SafeTxOverview";
import { useTransactionProposalDetails } from "@/hooks/useTransactionDetails";
import { bytes32Schema } from "@/lib/schemas";

const validateSearch = z.object({
	id: bytes32Schema.catch("0x"),
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
		<Container>
			<ConditionalBackButton />
			<ContainerTitle>Proposal Details</ContainerTitle>
			{details.isLoading && "Loading ..."}
			{details.data !== null && (
				<div className={"space-y-4"}>
					<Box>
						<TransactionProposalDetails proposal={details.data.proposal} hideProposedAt />
					</Box>
					<Box className={`${isAttested ? "border-positive" : "border-pending"}`}>
						<p className={"text-xs"}>Message: {details.data.proposal.message}</p>
						<p>Proposed at block {details.data.proposal.proposedAt}</p>
						{isAttested && <p>Attested at block {details.data.attestedAt}</p>}
						{!isAttested && <p>Attestetation pending</p>}
					</Box>
					<Box>
						<TransactionDataDetails data={details.data.proposal.transaction.data} />
					</Box>
				</div>
			)}
			{!details.isFetching && details.data === null && <Box>"Could not load proposal!"</Box>}
		</Container>
	);
}
