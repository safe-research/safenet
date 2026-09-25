// @vitest-environment jsdom

import type { DefinedUseQueryResult } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Address, Hex } from "viem";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Arbitration, SafeTransaction, TransactionProposalWithStatus } from "@/lib/consensus";
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

vi.mock("@/hooks/useVotingStatus", () => ({
	useVotingStatus: vi.fn(() => ({ data: null })),
}));

vi.mock("@/hooks/useSentinelVotes", () => ({
	useSentinelVotes: vi.fn(() => ({ data: [] })),
}));

vi.mock("@/hooks/useSentinelInfo", () => ({
	useSentinelInfoMap: vi.fn(() => ({ data: null })),
}));

vi.mock("@/hooks/useConsensusState", () => ({
	useConsensusState: vi.fn(() => ({ data: { currentBlock: 0n, chainId: 100n } })),
}));

vi.mock("@/hooks/useOracleArbitrator", () => ({
	useOracleArbitrator: vi.fn(() => ({ data: undefined })),
}));

vi.mock("../common/Info", () => ({
	InlineBlockInfo: ({ block }: { block: bigint }) => <span>{block.toString()}</span>,
	InlineExplorerTxLink: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
}));

import { useConsensusState } from "@/hooks/useConsensusState";
import { useOracleArbitrator } from "@/hooks/useOracleArbitrator";
import { useProposalsForTransaction } from "@/hooks/useProposalsForTransaction";
import { useSentinelVotes } from "@/hooks/useSentinelVotes";
import { useVotingStatus } from "@/hooks/useVotingStatus";

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
	oracle: "0x0000000000000000000000000000000000000099" as Address,
	oracleData: "0x" as Hex,
	requestId: `0x${"cd".repeat(32)}` as Hex,
	transaction: makeTransaction(),
	proposedAt: { block: 100n, tx: "0xabc" as Hex },
	attestedAt: null,
	status: "PROPOSED",
	arbitration: null,
	...overrides,
});

const makeArbitration = (outcome: Arbitration["outcome"] = null): Arbitration => ({
	triggeredAt: { block: 105n, tx: "0x105" as Hex },
	deadline: 200n,
	outcome,
});

const renderArbitration = (arbitration: Arbitration) => {
	vi.mocked(useProposalsForTransaction).mockReturnValue(
		mockQueryResult([makeProposal({ status: "ARBITRATING", arbitration })]),
	);
	render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction()} />);
};

// The text of the arbitration row with the given label, so assertions see the label and its value together.
const rowText = (label: string) => screen.getByText(label).parentElement?.textContent;

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

	it("shows an Oracle row with the derived status for an oracle proposal", () => {
		vi.mocked(useVotingStatus).mockReturnValue({ data: { kind: "generic", approved: true } } as never);
		vi.mocked(useProposalsForTransaction).mockReturnValue(
			mockQueryResult([makeProposal({ oracle: "0x0000000000000000000000000000000000000099" as Address })]),
		);
		render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction()} />);
		expect(screen.getByText("Oracle")).toBeTruthy();
		expect(screen.getByText("APPROVED")).toBeTruthy();
		expect(screen.queryByText("0x0000…0011 ⏳")).toBeNull();
	});

	it("shows the per-sentinel vote list for a sentinel oracle request", () => {
		vi.mocked(useVotingStatus).mockReturnValue({
			data: { kind: "sentinel", state: "PENDING", approveCount: 1n, denyCount: 0n },
		} as never);
		vi.mocked(useSentinelVotes).mockReturnValue({
			data: [
				{ sentinel: "0x0000000000000000000000000000000000000011" as Address, state: "committed" },
				{
					sentinel: "0x0000000000000000000000000000000000000022" as Address,
					state: "approved",
					reason: "looks fine",
				},
			],
		} as never);
		vi.mocked(useProposalsForTransaction).mockReturnValue(
			mockQueryResult([makeProposal({ oracle: "0x0000000000000000000000000000000000000099" as Address })]),
		);
		render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction()} />);
		expect(screen.getByText("0x0000…0011 ⏳")).toBeTruthy();
		expect(screen.getByText("0x0000…0022 ✅")).toBeTruthy();
		fireEvent.click(screen.getByText("0x0000…0022 ✅"));
		expect(screen.getByText("looks fine")).toBeTruthy();
	});

	describe("arbitration", () => {
		const ruledAt = { block: 300n, tx: "0x300" as Hex };

		it("shows no arbitration section for a proposal without an arbitration", () => {
			vi.mocked(useProposalsForTransaction).mockReturnValue(mockQueryResult([makeProposal()]));
			render(<SafeTxProposals safeTxHash={SAFE_TX_HASH} transaction={makeTransaction()} />);
			expect(screen.queryByText("Arbitration:")).toBeNull();
		});

		it("shows a pending dispute with its start, deadline and closing note", () => {
			vi.mocked(useConsensusState).mockReturnValue({ data: { currentBlock: 200n, chainId: 100n } } as never);
			renderArbitration(makeArbitration());
			expect(rowText("Started:")).toContain("105");
			expect(rowText("Deadline:")).toBe("Deadline:Block 200");
			expect(rowText("Outcome:")).toBe("Outcome:Pending");
			expect(screen.getByText(/Transactions that enter arbitration are never attested/)).toBeTruthy();
		});

		it("marks the deadline as passed once the current block is past it", () => {
			vi.mocked(useConsensusState).mockReturnValue({ data: { currentBlock: 201n, chainId: 100n } } as never);
			renderArbitration(makeArbitration());
			expect(rowText("Deadline:")).toBe("Deadline:Block 200 (passed)");
		});

		it("does not mark the deadline as passed once there is an outcome", () => {
			vi.mocked(useConsensusState).mockReturnValue({ data: { currentBlock: 400n, chainId: 100n } } as never);
			renderArbitration(makeArbitration({ kind: "timedOut", at: ruledAt }));
			expect(rowText("Deadline:")).toBe("Deadline:Block 200");
		});

		it.each<[string, NonNullable<Arbitration["outcome"]>]>([
			["Ruled secure", { kind: "ruled", secure: true, context: "", at: ruledAt }],
			["Ruled insecure", { kind: "ruled", secure: false, context: "", at: ruledAt }],
			["Out of scope", { kind: "outOfScope", context: "", at: ruledAt }],
			["Timed out without a ruling", { kind: "timedOut", at: ruledAt }],
		])("shows the %s outcome with its block", (label, outcome) => {
			renderArbitration(makeArbitration(outcome));
			expect(rowText("Outcome:")).toBe(`Outcome:${label}, 300 Explorer Tx`);
		});

		it.each<[string, NonNullable<Arbitration["outcome"]>]>([
			["a ruling", { kind: "ruled", secure: false, context: '<a href="x">phish</a> & <b>bold</b>', at: ruledAt }],
			["an out-of-scope decision", { kind: "outOfScope", context: '<a href="x">phish</a> & <b>bold</b>', at: ruledAt }],
		])("renders the reason for %s verbatim, markup included", (_, outcome) => {
			renderArbitration(makeArbitration(outcome));
			expect(rowText("Reason:")).toBe('Reason:<a href="x">phish</a> & <b>bold</b>');
			expect(screen.queryByText("phish")).toBeNull();
		});

		it("hides an empty reason", () => {
			renderArbitration(makeArbitration({ kind: "ruled", secure: true, context: "", at: ruledAt }));
			expect(screen.queryByText("Reason:")).toBeNull();
		});

		it("shows the arbitrator's address", () => {
			vi.mocked(useOracleArbitrator).mockReturnValue({
				data: "0xe682000000000000000000000000000000005cc8" as Address,
			} as never);
			renderArbitration(makeArbitration());
			expect(rowText("Arbitrator:")).toBe("Arbitrator:0xE682…5cC8");
		});
	});
});
