// @vitest-environment jsdom

import type { DefinedUseQueryResult } from "@tanstack/react-query";
import { cleanup, render, screen } from "@testing-library/react";
import type { Address, Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SafeTransaction, TransactionProposalWithStatus } from "@/lib/consensus";
import { SafeTxProposals } from "./SafeTxProposals";

const mockQueryResult = (data: TransactionProposalWithStatus[], isFetching = false) =>
	({ isFetching, data }) as unknown as DefinedUseQueryResult<TransactionProposalWithStatus[], Error>;

vi.mock("@/hooks/useProposalsForTransaction", () => ({
	useProposalsForTransaction: vi.fn(() => mockQueryResult([])),
}));

vi.mock("@/hooks/useSubmitProposal", () => ({
	useSubmitProposal: vi.fn(() => ({ enabled: false, mutation: { isSuccess: false, isPending: false, error: null } })),
}));

vi.mock("./SafeTxAttestationStatus", () => ({
	SafeTxAttestationStatus: () => null,
}));

vi.mock("@/hooks/useSigningProgress", () => ({
	useAttestationStatus: vi.fn(() => ({ data: null, isFetching: false })),
}));

vi.mock("../common/Info", () => ({
	InlineBlockInfo: ({ block }: { block: bigint }) => <span>{block.toString()}</span>,
	InlineExplorerTxLink: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
}));

import { useProposalsForTransaction } from "@/hooks/useProposalsForTransaction";

afterEach(cleanup);

const SAFE_TX_HASH = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef" as Hex;

const makeTransaction = (chainId = 8453n): SafeTransaction => ({
	chainId,
	safe: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" as Address,
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
});

const makeProposal = (overrides?: Partial<TransactionProposalWithStatus>): TransactionProposalWithStatus => ({
	chainId: 8453n,
	safeTxHash: SAFE_TX_HASH,
	epoch: 1n,
	transaction: makeTransaction(),
	proposedAt: { block: 100n, tx: "0xabc" as Hex },
	attestedAt: null,
	status: "PROPOSED",
	...overrides,
});

describe("SafeTxProposals", () => {
	it("labels an attested proposal as ATTESTED", () => {
		vi.mocked(useProposalsForTransaction).mockReturnValue(
			mockQueryResult([makeProposal({ status: "ATTESTED", attestedAt: { block: 110n, tx: "0xdef" as Hex } })]),
		);
		render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction()} />);
		expect(screen.getByText("ATTESTED")).toBeTruthy();
	});

	it("labels a timed-out proposal as TIMED OUT", () => {
		vi.mocked(useProposalsForTransaction).mockReturnValue(mockQueryResult([makeProposal({ status: "TIMED_OUT" })]));
		render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction()} />);
		expect(screen.getByText("TIMED OUT")).toBeTruthy();
	});

	it("labels an in-progress proposal as PROPOSED", () => {
		vi.mocked(useProposalsForTransaction).mockReturnValue(mockQueryResult([makeProposal({ status: "PROPOSED" })]));
		render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction()} />);
		expect(screen.getByText("PROPOSED")).toBeTruthy();
	});

	it("numbers proposals starting at Proposal #1", () => {
		vi.mocked(useProposalsForTransaction).mockReturnValue(
			mockQueryResult([makeProposal({ epoch: 1n }), makeProposal({ epoch: 2n, status: "ATTESTED" })]),
		);
		render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction()} />);
		expect(screen.getByText("Proposal #1")).toBeTruthy();
		expect(screen.getByText("Proposal #2")).toBeTruthy();
	});

	it("shows no-proposals message with chain name for a known chain", () => {
		vi.mocked(useProposalsForTransaction).mockReturnValue(mockQueryResult([]));
		render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction(8453n)} />);
		expect(screen.getByText(/No proposals found for this SafeTxHash on Base/)).toBeTruthy();
	});

	it("shows no-proposals message with raw chainId for an unknown chain", () => {
		vi.mocked(useProposalsForTransaction).mockReturnValue(mockQueryResult([]));
		render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction(99999n)} />);
		expect(screen.getByText(/No proposals found for this SafeTxHash on chain 99999/)).toBeTruthy();
	});
});
