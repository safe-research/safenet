// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { createElement } from "react";
import type { Address, PublicClient } from "viem";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "@/lib/settings";
import { useSafeTransactionProposals } from "./useSafeTransactionProposals";

const CONSENSUS = "0x0000000000000000000000000000000000000001" as Address;
const SAFE_ADDRESS = "0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF" as Address;
const CHAIN_ID = 1n;
const CURRENT_BLOCK = 5000n;
const MAX_BLOCK_RANGE = 1000;

const DEFAULT_SETTINGS: Settings = {
	consensus: CONSENSUS,
	rpc: "https://example.com",
	decoder: "https://example.com/decoder?calldata=",
	maxBlockRange: MAX_BLOCK_RANGE,
	validatorInfo: "https://example.com/validator-info.json",
	refetchInterval: 0,
};

// Mock hooks
vi.mock("@/hooks/useSettings", () => ({
	useSettings: vi.fn(() => [DEFAULT_SETTINGS]),
}));

const mockProvider: PublicClient = {
	getBlockNumber: vi.fn().mockResolvedValue(CURRENT_BLOCK),
	request: vi.fn().mockResolvedValue([]),
} as unknown as PublicClient;

vi.mock("@/hooks/useProvider", () => ({
	useProvider: vi.fn(() => mockProvider),
}));

const createWrapper = () => {
	const queryClient = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	return ({ children }: { children: React.ReactNode }) =>
		createElement(QueryClientProvider, { client: queryClient }, children);
};

beforeEach(() => {
	vi.clearAllMocks();
	(mockProvider.getBlockNumber as ReturnType<typeof vi.fn>).mockResolvedValue(CURRENT_BLOCK);
	(mockProvider.request as ReturnType<typeof vi.fn>).mockResolvedValue([]);
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe("useSafeTransactionProposals", () => {
	it("is disabled until the initial block is fetched", async () => {
		let resolveBlock: (v: bigint) => void;
		const blockPromise = new Promise<bigint>((resolve) => {
			resolveBlock = resolve;
		});
		(mockProvider.getBlockNumber as ReturnType<typeof vi.fn>).mockReturnValue(blockPromise);

		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		// Infinite query should not have fetched yet since initialFromBlock is not resolved
		expect(result.current.isFetchingNextPage).toBe(false);
		expect(mockProvider.request).not.toHaveBeenCalled();

		// Now resolve the initial block
		await act(async () => {
			resolveBlock?.(CURRENT_BLOCK);
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));
		expect(mockProvider.request).toHaveBeenCalled();
	});

	it("fetches the most recent block window on first load", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		// The request should have been called with fromBlock = currentBlock - maxBlockRange
		const requestCalls = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls;
		const logsCall = requestCalls.find((c) => c[0].method === "eth_getLogs");
		expect(logsCall).toBeDefined();
		const params = logsCall?.[0].params[0];
		// fromBlock = 5000 - 1000 = 4000
		expect(params.fromBlock).toBe("0xfa0");
	});

	it("filters by safe address via topics", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		const requestCalls = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls;
		const logsCall = requestCalls.find((c) => c[0].method === "eth_getLogs");
		expect(logsCall).toBeDefined();
		const topics = logsCall?.[0].params[0].topics;
		// topics[3] should be the safe address
		expect(topics[3]).toBe(SAFE_ADDRESS);
	});

	it("exposes flat list of proposals from all pages", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		// With empty mock responses, data.pages.flat() should be an empty array
		expect(result.current.data?.pages.flat()).toEqual([]);
	});

	it("hasNextPage is true when more blocks exist to fetch", async () => {
		// currentBlock=5000, maxBlockRange=1000 → initialFromBlock=4000 → nextFrom=3000 ≥ 0
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(result.current.hasNextPage).toBe(true);
	});

	it("hasNextPage is false when fromBlock would go below zero", async () => {
		// Set currentBlock such that initialFromBlock = 0
		(mockProvider.getBlockNumber as ReturnType<typeof vi.fn>).mockResolvedValue(500n);

		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		// initialFromBlock = 0 (since 500 < 1000), nextFrom = 0 - 1000 = -1000 < 0 → no next page
		expect(result.current.hasNextPage).toBe(false);
	});

	it("fetches the next block window when fetchNextPage is called", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		const requestCallsBefore = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls.filter(
			(c) => c[0].method === "eth_getLogs",
		).length;

		await act(async () => {
			result.current.fetchNextPage();
		});

		await waitFor(() => {
			const requestCallsAfter = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls.filter(
				(c) => c[0].method === "eth_getLogs",
			).length;
			expect(requestCallsAfter).toBeGreaterThan(requestCallsBefore);
		});

		// Second fetch should cover the previous window: fromBlock = 4000 - 1000 = 3000
		const allLogsCalls = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls.filter(
			(c) => c[0].method === "eth_getLogs",
		);
		const secondCallParams = allLogsCalls[1][0].params[0];
		expect(secondCallParams.fromBlock).toBe("0xbb8"); // 3000 in hex
	});
});
