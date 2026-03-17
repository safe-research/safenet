import { createFileRoute } from "@tanstack/react-router";
import z from "zod";
import { ConditionalBackButton } from "@/components/BackButton";
import { Box, Container, ContainerTitle } from "@/components/Groups";
import { Skeleton } from "@/components/Skeleton";
import { SafeTxHeader } from "@/components/transaction/SafeTxHeader";
import { SafeTxProposals } from "@/components/transaction/SafeTxProposals";
import { SafeTxSummary } from "@/components/transaction/SafeTxSummary";
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
			{details.isFetching && details.data === null && <Skeleton className="w-full h-25" />}
			{!details.isFetching && details.data === null && <Box>"Could not load proposal!"</Box>}
			{details.data !== null && (
				<>
					<Box>
						<SafeTxHeader safeTxHash={safeTxHash} transaction={details.data} fromSafeApi={details.fromSafeApi} />
					</Box>
					<Box>
						<SafeTxSummary transaction={details.data} />
					</Box>
					<Box>
						<SafeTxProposals safeTxHash={safeTxHash} transaction={details.data} />
					</Box>
				</>
			)}
		</Container>
	);
}
