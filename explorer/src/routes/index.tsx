import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { z } from "zod";
import { Container } from "@/components/Groups";
import { Skeleton } from "@/components/Skeleton";
import { SearchBar } from "@/components/search/SearchBar";
import { RecentTransactionProposals } from "@/components/transaction/RecentTransactionProposals";
import { useRecentTransactionProposals } from "@/hooks/useRecentTransactionProposals";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";

export function App() {
	return <AppInner />;
}

const PAGE_SIZE = 10;

function AppInner() {
	const [autoRefresh, setAutoRefresh] = useState(true);
	const proposals = useRecentTransactionProposals(autoRefresh);
	const { limit, network } = Route.useSearch();
	const navigate = useNavigate({ from: Route.fullPath });

	const handleShowMore = () => {
		navigate({
			search: (prev) => ({ ...prev, limit: (prev.limit ?? PAGE_SIZE) + PAGE_SIZE }),
			resetScroll: false,
			replace: true,
		});
	};

	const updateSelectedNetwork = (network: string) => {
		navigate({
			search: (prev) => ({ ...prev, network }),
			resetScroll: false,
			replace: true,
		});
	};

	return (
		<Container>
			<div className="text-center mb-12">
				<h1 className="text-3xl font-bold text-title sm:text-4xl mb-4">Safenet Explorer</h1>
				<p className="text-lg text-sub-title max-w-2xl mx-auto">Explore the future of transaction security!</p>
			</div>

			<SearchBar className="mb-8" onSelectNetwork={updateSelectedNetwork} selectedNetwork={network} />

			{proposals.data.length === 0 && proposals.isFetching && <Skeleton className="w-full h-25" />}
			{proposals.data.length > 0 && (
				<RecentTransactionProposals
					proposals={proposals.data}
					itemsToShow={limit ?? PAGE_SIZE}
					onShowMore={handleShowMore}
					isFetching={proposals.isFetching}
					dataUpdatedAt={proposals.dataUpdatedAt}
					autoRefresh={autoRefresh}
					onRefetch={() => proposals.refetch()}
					onToggleAutoRefresh={() => setAutoRefresh((prev) => !prev)}
				/>
			)}
		</Container>
	);
}

const entriesSearchSchema = z.object({
	network: z.coerce
		.string()
		.pipe(z.union(Object.keys(SAFE_SERVICE_CHAINS).map((c) => z.literal(c))).or(z.literal("Safenet")))
		.optional()
		.catch(undefined),
	limit: z.coerce.number().optional().catch(PAGE_SIZE),
});

export const Route = createFileRoute("/")({
	validateSearch: (search) => entriesSearchSchema.parse(search),
	component: App,
});
