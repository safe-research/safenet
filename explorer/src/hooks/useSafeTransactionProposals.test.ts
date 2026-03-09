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
	it("fetches the most recent block window on first load", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		// fromBlock = currentBlock - maxBlockRange = 5000 - 1000 = 4000 = 0xfa0
		const requestCalls = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls;
		const logsCall = requestCalls.find((c) => c[0].method === "eth_getLogs");
		expect(logsCall).toBeDefined();
		expect(logsCall?.[0].params[0].fromBlock).toBe("0xfa0");
	});

	it("anchors to the current block at fetch time via getBlockNumber", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(mockProvider.getBlockNumber).toHaveBeenCalled();
	});

	it("filters by safe address as the fourth topic", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		const logsCall = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls.find(
			(c) => c[0].method === "eth_getLogs",
		);
		expect(logsCall?.[0].params[0].topics[3]).toBe(SAFE_ADDRESS);
	});

	it("exposes a flat list of proposals across all pages", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(result.current.data?.pages.flat()).toEqual([]);
	});

	it("hasNextPage is true when more blocks exist to fetch", async () => {
		// initialFromBlock = 5000 - 1000 = 4000; nextFrom = 4000 - 1000 = 3000 ≥ 0
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(result.current.hasNextPage).toBe(true);
	});

	it("hasNextPage is false when the next window would start below block 0", async () => {
		// currentBlock=500 < maxBlockRange=1000 → fromBlock=0; nextFrom = 0 - 1000 < 0
		(mockProvider.getBlockNumber as ReturnType<typeof vi.fn>).mockResolvedValue(500n);

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

		const logCallsBefore = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls.filter(
			(c) => c[0].method === "eth_getLogs",
		).length;

		await act(async () => {
			result.current.fetchNextPage();
		});

		await waitFor(() => {
			const logCallsAfter = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls.filter(
				(c) => c[0].method === "eth_getLogs",
			).length;
			expect(logCallsAfter).toBeGreaterThan(logCallsBefore);
		});

		// Page 1: fromBlock = 4000 - 1000 = 3000 = 0xbb8
		const allLogsCalls = (mockProvider.request as ReturnType<typeof vi.fn>).mock.calls.filter(
			(c) => c[0].method === "eth_getLogs",
		);
		expect(allLogsCalls[1][0].params[0].fromBlock).toBe("0xbb8");
	});
});
