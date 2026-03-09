import type { PublicClient } from "viem";
import { describe, expect, it, vi } from "vitest";
import { getBlockRange, mostRecentFirst } from "./utils";

const CURRENT_BLOCK = 10000n;
const MAX_BLOCK_RANGE = 1000n;

const makeProvider = (blockNumber = CURRENT_BLOCK): PublicClient =>
	({ getBlockNumber: vi.fn().mockResolvedValue(blockNumber) }) as unknown as PublicClient;

describe("getBlockRange", () => {
	it("fetches the current block when referenceBlock is not provided", async () => {
		const provider = makeProvider();
		const { toBlock } = await getBlockRange(provider, MAX_BLOCK_RANGE);
		expect(provider.getBlockNumber).toHaveBeenCalledOnce();
		expect(toBlock).toBe(CURRENT_BLOCK);
	});

	it("uses referenceBlock as toBlock without calling getBlockNumber", async () => {
		const provider = makeProvider();
		const { toBlock } = await getBlockRange(provider, MAX_BLOCK_RANGE, 6000n);
		expect(provider.getBlockNumber).not.toHaveBeenCalled();
		expect(toBlock).toBe(6000n);
	});

	it("computes fromBlock as toBlock - maxBlockRange", async () => {
		const provider = makeProvider();
		const { fromBlock } = await getBlockRange(provider, MAX_BLOCK_RANGE);
		expect(fromBlock).toBe(CURRENT_BLOCK - MAX_BLOCK_RANGE);
	});

	it("clamps fromBlock to 0 when toBlock is less than maxBlockRange", async () => {
		const provider = makeProvider(500n);
		const { fromBlock, toBlock } = await getBlockRange(provider, MAX_BLOCK_RANGE);
		expect(toBlock).toBe(500n);
		expect(fromBlock).toBe(0n);
	});
});

describe("mostRecentFirst", () => {
	it("sorts logs by blockNumber descending", () => {
		const logs = [
			{ blockNumber: 100n, logIndex: 0 },
			{ blockNumber: 300n, logIndex: 0 },
			{ blockNumber: 200n, logIndex: 0 },
		];
		const sorted = mostRecentFirst(logs);
		expect(sorted.map((l) => l.blockNumber)).toEqual([300n, 200n, 100n]);
	});

	it("sorts by logIndex descending when blockNumbers are equal", () => {
		const logs = [
			{ blockNumber: 100n, logIndex: 1 },
			{ blockNumber: 100n, logIndex: 3 },
			{ blockNumber: 100n, logIndex: 2 },
		];
		const sorted = mostRecentFirst(logs);
		expect(sorted.map((l) => l.logIndex)).toEqual([3, 2, 1]);
	});

	it("returns empty array for empty input", () => {
		expect(mostRecentFirst([])).toEqual([]);
	});

	it("handles single element", () => {
		const logs = [{ blockNumber: 42n, logIndex: 0 }];
		expect(mostRecentFirst(logs)).toEqual([{ blockNumber: 42n, logIndex: 0 }]);
	});

	it("sorts by blockNumber first, then logIndex", () => {
		const logs = [
			{ blockNumber: 100n, logIndex: 2 },
			{ blockNumber: 200n, logIndex: 0 },
			{ blockNumber: 100n, logIndex: 5 },
			{ blockNumber: 200n, logIndex: 1 },
		];
		const sorted = mostRecentFirst(logs);
		expect(sorted).toEqual([
			{ blockNumber: 200n, logIndex: 1 },
			{ blockNumber: 200n, logIndex: 0 },
			{ blockNumber: 100n, logIndex: 5 },
			{ blockNumber: 100n, logIndex: 2 },
		]);
	});
});
