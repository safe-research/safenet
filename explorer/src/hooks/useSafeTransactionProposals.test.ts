// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { createElement } from "react";
import type { Address, PublicClient } from "viem";
import { numberToHex } from "viem";
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

const logsCalls = () =>
	(mockProvider.request as ReturnType<typeof vi.fn>).mock.calls.filter((c) => c[0].method === "eth_getLogs");

beforeEach(() => {
	vi.clearAllMocks();
	(mockProvider.getBlockNumber as ReturnType<typeof vi.fn>).mockResolvedValue(CURRENT_BLOCK);
	(mockProvider.request as ReturnType<typeof vi.fn>).mockResolvedValue([]);
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

		expect(mockProvider.getBlockNumber).toHaveBeenCalled();
		// toBlock = 5000 = 0x1388
		expect(logsCalls()[0][0].params[0].toBlock).toBe(numberToHex(CURRENT_BLOCK));
		// fromBlock = 5000 - 1000 = 4000 = 0xfa0
		expect(logsCalls()[0][0].params[0].fromBlock).toBe(numberToHex(CURRENT_BLOCK - BigInt(MAX_BLOCK_RANGE)));
	});

	it("filters by safe address as the fourth topic", async () => {
		const { result } = renderHook(() => useSafeTransactionProposals({ safeAddress: SAFE_ADDRESS, chainId: CHAIN_ID }), {
			wrapper: createWrapper(),
		});

		await waitFor(() => expect(result.current.isSuccess).toBe(true));

		expect(logsCalls()[0][0].params[0].topics[3]).toBe("0x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
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
		// currentBlock=500 < maxBlockRange=1000 → fromBlock=0; 0 > 0 is false → no next page
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

		await act(async () => {
			result.current.fetchNextPage();
		});

		await waitFor(() => expect(logsCalls().length).toBe(2));

		// Page 1: toBlock = fromBlock_p0 - 1 = 4000 - 1 = 3999; fromBlock = 3999 - 1000 = 2999
		const p1Params = logsCalls()[1][0].params[0];
		expect(p1Params.toBlock).toBe(numberToHex(3999n));
		expect(p1Params.fromBlock).toBe(numberToHex(2999n));
	});
});
