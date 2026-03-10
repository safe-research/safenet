import { createFileRoute } from "@tanstack/react-router";
import z from "zod";
import { ConditionalBackButton } from "@/components/BackButton";
import { Container, ContainerTitle } from "@/components/Groups";
import { Skeleton } from "@/components/Skeleton";
import { TransactionProposalsList } from "@/components/transaction/TransactionProposalsList";
import { useSafeTransactionProposals } from "@/hooks/useSafeTransactionProposals";
import { bigIntSchema, checkedAddressSchema } from "@/lib/schemas";

const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000" as const;

const validateSearch = z.object({
	safeAddress: checkedAddressSchema.catch(ZERO_ADDRESS),
	chainId: bigIntSchema.catch(1n),
});

export const Route = createFileRoute("/safe")({
	validateSearch,
	component: SafePage,
});

export function SafePage() {
	const { safeAddress, chainId } = Route.useSearch();
	const { data, isFetching, isFetchingNextPage, hasNextPage, fetchNextPage } = useSafeTransactionProposals({
		safeAddress,
		chainId,
	});

	const proposals = data?.pages.flat() ?? [];
	const isFirstLoad = isFetching && data === undefined;

	return (
		<Container className="space-y-4">
			<ConditionalBackButton />
			<ContainerTitle>Proposals for {safeAddress}</ContainerTitle>
			{isFirstLoad && <Skeleton className="w-full h-25" />}
			{!isFirstLoad && proposals.length === 0 && (
				<p className="text-center text-sub-title py-8">No proposals found for this Safe address.</p>
			)}
			{proposals.length > 0 && (
				<TransactionProposalsList
					proposals={proposals}
					hasMore={hasNextPage}
					onShowMore={fetchNextPage}
					isLoadingMore={isFetchingNextPage}
					showMoreLabel="Load More"
				/>
			)}
		</Container>
	);
}
