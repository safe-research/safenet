// @vitest-environment jsdom
import type { InfiniteData } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Address, Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TransactionProposal } from "@/lib/consensus";

const mockUseSearch = vi.hoisted(() =>
	vi.fn(() => ({
		safeAddress: "0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF" as Address,
		chainId: 1n,
	})),
);

vi.mock("@tanstack/react-router", () => ({
	createFileRoute: () => (options: unknown) => ({ useSearch: mockUseSearch, ...(options as object) }),
	useCanGoBack: vi.fn(() => false),
	useRouter: vi.fn(() => ({ history: { back: vi.fn() }, navigate: vi.fn() })),
	Link: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
}));

const mockFetchNextPage = vi.fn();

type HookResult = {
	data: InfiniteData<TransactionProposal[]> | undefined;
	isFetching: boolean;
	isFetchingNextPage: boolean;
	hasNextPage: boolean;
	fetchNextPage: () => void;
};

const mockUseSafeTransactionProposals = vi.hoisted(() => vi.fn<() => HookResult>());

vi.mock("@/hooks/useSafeTransactionProposals", () => ({
	useSafeTransactionProposals: mockUseSafeTransactionProposals,
}));

vi.mock("@/components/transaction/TransactionProposalsList", () => ({
	TransactionProposalsList: ({
		proposals,
		hasMore,
		onShowMore,
		isLoadingMore,
		showMoreLabel,
	}: {
		proposals: TransactionProposal[];
		hasMore: boolean;
		onShowMore: () => void;
		isLoadingMore?: boolean;
		showMoreLabel?: string;
	}) => (
		<div data-testid="proposals-list">
			{proposals.map((p) => (
				<div key={p.safeTxHash}>{p.safeTxHash}</div>
			))}
			{hasMore && (
				<button type="button" onClick={onShowMore} disabled={isLoadingMore}>
					{isLoadingMore ? "Loading" : (showMoreLabel ?? "Load More")}
				</button>
			)}
		</div>
	),
}));

afterEach(() => {
	cleanup();
	mockUseSafeTransactionProposals.mockClear();
	mockFetchNextPage.mockClear();
});

const makeProposal = (safeTxHash: string): TransactionProposal => ({
	chainId: 1n,
	safeTxHash: safeTxHash as Hex,
	epoch: 1n,
	transaction: {
		chainId: 1n,
		safe: "0x0000000000000000000000000000000000000001" as Address,
		to: "0x0000000000000000000000000000000000000002" as Address,
		value: 0n,
		data: "0x" as Hex,
		operation: 0,
		safeTxGas: 0n,
		baseGas: 0n,
		gasPrice: 0n,
		gasToken: "0x0000000000000000000000000000000000000000" as Address,
		refundReceiver: "0x0000000000000000000000000000000000000000" as Address,
		nonce: 0n,
	},
	proposedAt: { block: 100n, tx: "0xabc" as Hex },
	attestedAt: null,
});

describe("SafePage", () => {
	it("shows skeleton while first page is loading", async () => {
		mockUseSafeTransactionProposals.mockReturnValue({
			data: undefined,
			isFetching: true,
			isFetchingNextPage: false,
			hasNextPage: false,
			fetchNextPage: mockFetchNextPage,
		});

		const { SafePage } = await import("./safe");
		render(<SafePage />);

		expect(document.querySelector(".animate-pulse")).toBeTruthy();
		expect(screen.queryByTestId("proposals-list")).toBeNull();
	});

	it("renders proposals list when data is available", async () => {
		const proposals = [makeProposal("0xhash1"), makeProposal("0xhash2")];
		mockUseSafeTransactionProposals.mockReturnValue({
			data: { pages: [proposals], pageParams: [] },
			isFetching: false,
			isFetchingNextPage: false,
			hasNextPage: false,
			fetchNextPage: mockFetchNextPage,
		});

		const { SafePage } = await import("./safe");
		render(<SafePage />);

		expect(screen.getByTestId("proposals-list")).toBeTruthy();
		expect(screen.getByText("0xhash1")).toBeTruthy();
		expect(screen.getByText("0xhash2")).toBeTruthy();
	});

	it("shows empty state when loaded with no proposals", async () => {
		mockUseSafeTransactionProposals.mockReturnValue({
			data: { pages: [[]], pageParams: [] },
			isFetching: false,
			isFetchingNextPage: false,
			hasNextPage: false,
			fetchNextPage: mockFetchNextPage,
		});

		const { SafePage } = await import("./safe");
		render(<SafePage />);

		expect(screen.getByText(/no proposals found/i)).toBeTruthy();
		expect(screen.queryByTestId("proposals-list")).toBeNull();
	});

	it("Load More button calls fetchNextPage", async () => {
		const proposals = [makeProposal("0xhash1")];
		mockUseSafeTransactionProposals.mockReturnValue({
			data: { pages: [proposals], pageParams: [] },
			isFetching: false,
			isFetchingNextPage: false,
			hasNextPage: true,
			fetchNextPage: mockFetchNextPage,
		});

		const { SafePage } = await import("./safe");
		render(<SafePage />);

		fireEvent.click(screen.getByRole("button", { name: "Load More" }));
		expect(mockFetchNextPage).toHaveBeenCalledOnce();
	});

	it("does not show Load More button when hasNextPage is false", async () => {
		const proposals = [makeProposal("0xhash1")];
		mockUseSafeTransactionProposals.mockReturnValue({
			data: { pages: [proposals], pageParams: [] },
			isFetching: false,
			isFetchingNextPage: false,
			hasNextPage: false,
			fetchNextPage: mockFetchNextPage,
		});

		const { SafePage } = await import("./safe");
		render(<SafePage />);

		expect(screen.queryByRole("button", { name: "Load More" })).toBeNull();
	});

	it("flattens proposals across multiple pages", async () => {
		const page1 = [makeProposal("0xhash1")];
		const page2 = [makeProposal("0xhash2"), makeProposal("0xhash3")];
		mockUseSafeTransactionProposals.mockReturnValue({
			data: { pages: [page1, page2], pageParams: [] },
			isFetching: false,
			isFetchingNextPage: false,
			hasNextPage: false,
			fetchNextPage: mockFetchNextPage,
		});

		const { SafePage } = await import("./safe");
		render(<SafePage />);

		expect(screen.getByText("0xhash1")).toBeTruthy();
		expect(screen.getByText("0xhash2")).toBeTruthy();
		expect(screen.getByText("0xhash3")).toBeTruthy();
	});
});
