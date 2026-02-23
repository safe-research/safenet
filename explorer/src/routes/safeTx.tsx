import { createFileRoute } from "@tanstack/react-router";
import z from "zod";
import { ConditionalBackButton } from "@/components/BackButton";
import { Box, Container, ContainerTitle } from "@/components/Groups";
import { SafeTxDataDetails } from "@/components/transaction/SafeTxDataDetails";
import { SafeTxOverview } from "@/components/transaction/SafeTxOverview";
import { SafeTxProposals } from "@/components/transaction/SafeTxProposals";

import { useSafeTransactionDetails } from "@/hooks/useSafeTransactionDetails";
import { bigIntSchema, bytes32Schema } from "@/lib/schemas";

const validateSearch = z.object({
	chainId: bigIntSchema.catch(1n),
	safeTxHash: bytes32Schema.catch("0x"),
});

export const Route = createFileRoute("/safeTx")({
	validateSearch,
	component: SafeTransaction,
});

export function SafeTransaction() {
	const { chainId, safeTxHash } = Route.useSearch();
	const details = useSafeTransactionDetails(chainId, safeTxHash);
	return (
		<Container className="space-y-4">
			<ConditionalBackButton />
			<ContainerTitle>Transaction Details</ContainerTitle>
			{details.isLoading && "Loading ..."}
			{details.data !== null && (
				<>
					<Box>
						<SafeTxOverview title={`Safe Tx Hash: ${safeTxHash}`} transaction={details.data} />
					</Box>
					<Box>
						<SafeTxDataDetails data={details.data.data} />
					</Box>
					<Box>
						<SafeTxProposals transaction={details.data} />
					</Box>
				</>
			)}
			{!details.isFetching && details.data === null && <Box>"Could not load proposal!"</Box>}
		</Container>
	);
}
