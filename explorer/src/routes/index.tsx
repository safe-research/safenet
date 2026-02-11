import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { z } from "zod";
import { Container } from "@/components/Groups";
import { SearchBar } from "@/components/search/SearchBar";
import { TransactionProposalList } from "@/components/transaction/Proposals";
import { useRecentTransactionProposals } from "@/hooks/useRecentTransactions";
import { SAFE_SERVICE_CHAINS } from "@/lib/chains";

export function App() {
	return <AppInner />;
}

const PAGE_SIZE = 10;

function AppInner() {
	const recentProposals = useRecentTransactionProposals();
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

			{recentProposals.data.length > 0 && (
				<TransactionProposalList
					proposals={recentProposals.data}
					itemsToDisplay={limit ?? PAGE_SIZE}
					onShowMore={handleShowMore}
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
