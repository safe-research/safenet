import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import z from "zod";
import { ConditionalBackButton } from "@/components/BackButton";
import { Container, ContainerTitle } from "@/components/Groups";
import { Skeleton } from "@/components/Skeleton";
import { TransactionListControls } from "@/components/transaction/TransactionListControls";
import { TransactionProposalsList } from "@/components/transaction/TransactionProposalsList";
import { useSafeTransactionProposals } from "@/hooks/useSafeTransactionProposals";
import { zeroAddress } from "viem";
import { bigIntSchema, checkedAddressSchema } from "@/lib/schemas";

const validateSearch = z.object({
	safeAddress: checkedAddressSchema.catch(zeroAddress),
	chainId: bigIntSchema.catch(1n),
});

export const Route = createFileRoute("/safe")({
	validateSearch,
	component: SafePage,
});

export function SafePage() {
	const { safeAddress, chainId } = Route.useSearch();
	const [autoRefresh, setAutoRefresh] = useState(false);
	const { data, isFetching, isFetchingNextPage, hasNextPage, fetchNextPage, refetch, dataUpdatedAt } =
		useSafeTransactionProposals({ safeAddress, chainId, autoRefresh });

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
				<>
					<TransactionListControls
						isFetching={isFetching}
						dataUpdatedAt={dataUpdatedAt}
						autoRefresh={autoRefresh}
						onRefetch={refetch}
						onToggleAutoRefresh={() => setAutoRefresh((prev) => !prev)}
					/>
					<TransactionProposalsList
						proposals={proposals}
						hasMore={hasNextPage}
						onShowMore={fetchNextPage}
						isLoadingMore={isFetchingNextPage}
						showMoreLabel="Load More"
					/>
				</>
			)}
		</Container>
	);
}
