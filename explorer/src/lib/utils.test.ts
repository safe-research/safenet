import type { PublicClient } from "viem";
import { describe, expect, it, vi } from "vitest";
import { getFromBlock, mostRecentFirst } from "./utils";

describe("getFromBlock", () => {
	const makeProvider = (blockNumber: bigint): PublicClient =>
		({
			getBlockNumber: vi.fn().mockResolvedValue(blockNumber),
		}) as unknown as PublicClient;

	it("subtracts maxBlockRange from current block", async () => {
		const result = await getFromBlock(makeProvider(1000n), 200n);
		expect(result).toBe(800n);
	});

	it("returns 0n when block number is less than maxBlockRange", async () => {
		const result = await getFromBlock(makeProvider(50n), 200n);
		expect(result).toBe(0n);
	});

	it("returns 0n when block number equals maxBlockRange", async () => {
		const result = await getFromBlock(makeProvider(200n), 200n);
		expect(result).toBe(0n);
	});

	it("uses referenceBlock instead of fetching when provided", async () => {
		const provider = makeProvider(9999n);
		const result = await getFromBlock(provider, 100n, 500n);
		expect(result).toBe(400n);
		expect(provider.getBlockNumber).not.toHaveBeenCalled();
	});

	it("returns 0n when referenceBlock is less than maxBlockRange", async () => {
		const result = await getFromBlock(makeProvider(9999n), 100n, 50n);
		expect(result).toBe(0n);
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
