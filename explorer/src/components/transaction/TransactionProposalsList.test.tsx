// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Address, Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { TransactionProposalWithStatus } from "@/lib/consensus";
import { RecentTransactionProposals } from "./RecentTransactionProposals";
import { TransactionProposalsList } from "./TransactionProposalsList";

vi.mock("@/components/transaction/TransactionListRow", async (importOriginal) => {
	const actual = await importOriginal<typeof import("@/components/transaction/TransactionListRow")>();
	return {
		...actual,
		TransactionListRow: ({ proposal }: { proposal: TransactionProposalWithStatus }) => (
			<div data-testid="transaction-list-row">{proposal.safeTxHash}</div>
		),
		TransactionListRowSkeleton: () => <div data-testid="transaction-list-row-skeleton" />,
	};
});

afterEach(cleanup);

const makeProposal = (safeTxHash: string, epoch = 1n): TransactionProposalWithStatus => ({
	chainId: 1n,
	safeTxHash: safeTxHash as Hex,
	epoch,
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
	status: "PROPOSED",
});

const PROPOSALS = [makeProposal("0xhash1"), makeProposal("0xhash2"), makeProposal("0xhash3")];

describe("TransactionProposalsList", () => {
	it("renders the label count with 'recent proposals' suffix", () => {
		render(<TransactionProposalsList proposals={PROPOSALS} label="3" hasMore={false} onShowMore={vi.fn()} />);
		expect(screen.getByText(/3\s+recent proposals/)).toBeTruthy();
	});

	it("does not render a label row when label is omitted", () => {
		const { container } = render(
			<TransactionProposalsList proposals={PROPOSALS} hasMore={false} onShowMore={vi.fn()} />,
		);
		expect(container.querySelector(".text-xs.text-right")).toBeNull();
	});

	it("hides the show-more button when hasMore is false", () => {
		render(
			<TransactionProposalsList
				proposals={PROPOSALS}
				label="proposals"
				hasMore={false}
				onShowMore={vi.fn()}
				showMoreLabel="Load More"
			/>,
		);
		expect(screen.queryByRole("button")).toBeNull();
	});

	it("shows the show-more button when hasMore is true", () => {
		render(
			<TransactionProposalsList
				proposals={PROPOSALS}
				label="proposals"
				hasMore={true}
				onShowMore={vi.fn()}
				showMoreLabel="Load More"
			/>,
		);
		expect(screen.getByRole("button", { name: "Load More" })).toBeTruthy();
	});

	it("uses 'Show More' as default showMoreLabel", () => {
		render(<TransactionProposalsList proposals={PROPOSALS} label="proposals" hasMore={true} onShowMore={vi.fn()} />);
		expect(screen.getByRole("button", { name: "Show More" })).toBeTruthy();
	});

	it("calls onShowMore when the button is clicked", () => {
		const onShowMore = vi.fn();
		render(<TransactionProposalsList proposals={PROPOSALS} label="proposals" hasMore={true} onShowMore={onShowMore} />);
		fireEvent.click(screen.getByRole("button"));
		expect(onShowMore).toHaveBeenCalledOnce();
	});

	it("shows loading indicator and disables button when isLoadingMore is true", () => {
		render(
			<TransactionProposalsList
				proposals={PROPOSALS}
				label="proposals"
				hasMore={true}
				onShowMore={vi.fn()}
				isLoadingMore={true}
			/>,
		);
		const button = screen.getByRole("button") as HTMLButtonElement;
		expect(button.disabled).toBe(true);
		expect(screen.getByLabelText("Loading")).toBeTruthy();
	});

	it("renders a row for each proposal", () => {
		render(<TransactionProposalsList proposals={PROPOSALS} label="proposals" hasMore={false} onShowMore={vi.fn()} />);
		expect(screen.getAllByTestId("transaction-list-row")).toHaveLength(3);
	});

	it("renders each proposal's safeTxHash via TransactionListRow", () => {
		render(<TransactionProposalsList proposals={PROPOSALS} label="proposals" hasMore={false} onShowMore={vi.fn()} />);
		expect(screen.getByText("0xhash1")).toBeTruthy();
		expect(screen.getByText("0xhash2")).toBeTruthy();
		expect(screen.getByText("0xhash3")).toBeTruthy();
	});

	it("renders a single skeleton row instead of proposals when isLoading is true", () => {
		render(<TransactionProposalsList proposals={[]} hasMore={false} onShowMore={vi.fn()} isLoading={true} />);
		expect(screen.getAllByTestId("transaction-list-row-skeleton")).toHaveLength(1);
		expect(screen.queryAllByTestId("transaction-list-row")).toHaveLength(0);
	});

	it("renders the default empty label when not loading and proposals are empty", () => {
		render(<TransactionProposalsList proposals={[]} hasMore={false} onShowMore={vi.fn()} />);
		expect(screen.getByText("No transactions found")).toBeTruthy();
	});

	it("renders a custom emptyLabel when provided and proposals are empty", () => {
		render(
			<TransactionProposalsList
				proposals={[]}
				hasMore={false}
				onShowMore={vi.fn()}
				emptyLabel="No proposals for this Safe."
			/>,
		);
		expect(screen.getByText("No proposals for this Safe.")).toBeTruthy();
	});

	it("always renders the header row", () => {
		const { rerender } = render(
			<TransactionProposalsList proposals={[]} hasMore={false} onShowMore={vi.fn()} isLoading={true} />,
		);
		expect(screen.getByText("Network")).toBeTruthy();

		rerender(<TransactionProposalsList proposals={PROPOSALS} hasMore={false} onShowMore={vi.fn()} />);
		expect(screen.getByText("Network")).toBeTruthy();
	});

	it("hides the show-more button while isLoading is true", () => {
		render(<TransactionProposalsList proposals={[]} hasMore={true} onShowMore={vi.fn()} isLoading={true} />);
		expect(screen.queryByRole("button")).toBeNull();
	});
});

describe("RecentTransactionProposals", () => {
	const controlsProps = {
		isFetching: false,
		dataUpdatedAt: 0,
		autoRefresh: true,
		onRefetch: vi.fn(),
		onToggleAutoRefresh: vi.fn(),
	};

	it("renders the count and 'recent proposals' label", () => {
		render(
			<RecentTransactionProposals proposals={PROPOSALS} itemsToShow={10} onShowMore={vi.fn()} {...controlsProps} />,
		);
		expect(screen.getByText("3 recent proposals")).toBeTruthy();
	});

	it("always renders TransactionListControls, including while loading", () => {
		render(
			<RecentTransactionProposals
				proposals={[]}
				itemsToShow={10}
				onShowMore={vi.fn()}
				{...controlsProps}
				isLoading={true}
			/>,
		);
		expect(screen.getByRole("button", { name: /refresh now/i })).toBeTruthy();
	});

	it("renders an empty label row while loading to prevent layout shift", () => {
		const { container } = render(
			<RecentTransactionProposals
				proposals={[]}
				itemsToShow={10}
				onShowMore={vi.fn()}
				{...controlsProps}
				isLoading={true}
			/>,
		);
		// The label div must exist (even if empty) so the layout doesn't jump when data arrives
		expect(container.querySelector(".text-xs.text-right")).not.toBeNull();
	});

	it("shows total count in label even when proposals are sliced", () => {
		render(
			<RecentTransactionProposals proposals={PROPOSALS} itemsToShow={2} onShowMore={vi.fn()} {...controlsProps} />,
		);
		expect(screen.getByText("3 recent proposals")).toBeTruthy();
	});

	it("shows only itemsToShow proposals", () => {
		render(
			<RecentTransactionProposals proposals={PROPOSALS} itemsToShow={2} onShowMore={vi.fn()} {...controlsProps} />,
		);
		const rows = screen.getAllByTestId("transaction-list-row");
		expect(rows).toHaveLength(2);
	});

	it("shows 'Show More' button when there are more proposals than itemsToShow", () => {
		render(
			<RecentTransactionProposals proposals={PROPOSALS} itemsToShow={2} onShowMore={vi.fn()} {...controlsProps} />,
		);
		expect(screen.getByRole("button", { name: "Show More" })).toBeTruthy();
	});

	it("hides the 'Show More' button when all proposals fit within itemsToShow", () => {
		render(
			<RecentTransactionProposals proposals={PROPOSALS} itemsToShow={10} onShowMore={vi.fn()} {...controlsProps} />,
		);
		expect(screen.queryByRole("button", { name: "Show More" })).toBeNull();
	});

	it("calls onShowMore when 'Show More' is clicked", () => {
		const onShowMore = vi.fn();
		render(
			<RecentTransactionProposals proposals={PROPOSALS} itemsToShow={2} onShowMore={onShowMore} {...controlsProps} />,
		);
		fireEvent.click(screen.getByRole("button", { name: "Show More" }));
		expect(onShowMore).toHaveBeenCalledOnce();
	});
});
