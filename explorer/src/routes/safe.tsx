import { createFileRoute } from "@tanstack/react-router";
import { useState } from "react";
import { zeroAddress } from "viem";
import z from "zod";
import { ConditionalBackButton } from "@/components/BackButton";
import { Container, ContainerTitle } from "@/components/Groups";
import { TransactionListControls } from "@/components/transaction/TransactionListControls";
import { TransactionProposalsList } from "@/components/transaction/TransactionProposalsList";
import { useSafeTransactionProposals } from "@/hooks/useSafeTransactionProposals";
import { shortAddress } from "@/lib/address";
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
			<ContainerTitle>Proposals for {shortAddress(safeAddress)}</ContainerTitle>
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
				isLoading={isFirstLoad}
				isLoadingMore={isFetchingNextPage}
				showMoreLabel="Load More"
				emptyLabel="No proposals found for this Safe address."
			/>
		</Container>
	);
}
