// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { createElement } from "react";
import type { Address } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "@/lib/settings";
import { useSafeTransactionProposals } from "./useSafeTransactionProposals";

const CONSENSUS = "0x0000000000000000000000000000000000000001" as Address;
const SAFE_ADDRESS = "0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF" as Address;
const CHAIN_ID = 1n;
const CURRENT_BLOCK = 5000n;
const MAX_BLOCK_RANGE = 1000;
const RPC = "https://example.com";

const DEFAULT_SETTINGS: Settings = {
	consensus: CONSENSUS,
	rpc: RPC,
	decoder: "https://example.com/decoder?calldata=",
	maxBlockRange: MAX_BLOCK_RANGE,
	validatorInfo: "https://example.com/validator-info.json",
	refetchInterval: 0,
	blocksPerEpoch: 1440,
	signingTimeout: 12,
};

vi.mock("@/hooks/useSettings", () => ({
	useSettings: vi.fn(() => [DEFAULT_SETTINGS]),
}));

const mockLoadTransactionProposals = vi.fn();

vi.mock("@/lib/consensus", async (importOriginal) => {
	const actual = await importOriginal<typeof import("@/lib/consensus")>();
	return {
		...actual,
		getConsensusWorker: () => ({ loadTransactionProposals: mockLoadTransactionProposals }),
	};
});

const makeResult = (fromBlock: bigint, toBlock: bigint) => Promise.resolve({ proposals: [], fromBlock, toBlock });

const createWrapper = () => {
	const queryClient = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return ({ children }: { children: React.ReactNode }) =>
		createElement(QueryClientProvider, { client: queryClient }, children);
};

beforeEach(() => {
	vi.clearAllMocks();
	mockLoadTransactionProposals.mockImplementation(({ toBlock }: { toBlock?: bigint }) => {
		const to = toBlock ?? CURRENT_BLOCK;
		const from = to > BigInt(MAX_BLOCK_RANGE) ? to - BigInt(MAX_BLOCK_RANGE) : 0n;
		return makeResult(from, to);
	});
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe("useSafeTransactionProposals", () => {
	it("resolves toBlock from the current block on first load", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(mockLoadTransactionProposals).toHaveBeenCalledWith(
			expect.objectContaining({ rpc: RPC, consensus: CONSENSUS, toBlock: undefined }),
		);
	});

	it("filters by safe address in params", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(mockLoadTransactionProposals).toHaveBeenCalledWith(expect.objectContaining({ safe: SAFE_ADDRESS }));
	});

	it("exposes a flat list of proposals across all pages", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(result.current.data?.pages.flat()).toEqual([]);
	});

	it("hasNextPage is true when more blocks exist to fetch", async () => {
		// fromBlock = 5000 - 1000 = 4000 > 0 → next toBlock = 3999
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(result.current.hasNextPage).toBe(true);
	});

	it("hasNextPage is false when fromBlock reaches 0", async () => {
		// currentBlock < maxBlockRange → fromBlock=0; 0 > 0 is false → no next page
		mockLoadTransactionProposals.mockResolvedValue(makeResult(0n, 500n));

		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(result.current.hasNextPage).toBe(false);
	});

	it("fetches the previous block window when fetchNextPage is called", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		await act(async () => {
			result.current.fetchNextPage();
		});

		// Page 1 toBlock = fromBlock_p0 - 1 = 4000 - 1 = 3999
		await waitFor(() => expect(mockLoadTransactionProposals).toHaveBeenCalledTimes(2));

		expect(mockLoadTransactionProposals).toHaveBeenNthCalledWith(2, expect.objectContaining({ toBlock: 3999n }));
	});
});
