import type { PublicClient } from "viem";
import { describe, expect, it, vi } from "vitest";
import { estimateBlockAt, getBlockRange, getTargetedBlockRange, loadChainId, mostRecentFirst } from "./utils";

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

describe("loadChainId", () => {
	const makeChainProvider = (chainId = 1) =>
		({ getChainId: vi.fn().mockResolvedValue(chainId) }) as unknown as PublicClient;

	it("fetches and returns the chain id", async () => {
		const provider = makeChainProvider(5);
		await expect(loadChainId(provider)).resolves.toBe(5);
	});

	it("caches the result for the same provider instance", async () => {
		const provider = makeChainProvider(5);
		await loadChainId(provider);
		await loadChainId(provider);
		expect(provider.getChainId).toHaveBeenCalledOnce();
	});

	it("refetches when called with a different provider instance", async () => {
		const providerA = makeChainProvider(1);
		const providerB = makeChainProvider(2);
		await expect(loadChainId(providerA)).resolves.toBe(1);
		await expect(loadChainId(providerB)).resolves.toBe(2);
		expect(providerA.getChainId).toHaveBeenCalledOnce();
		expect(providerB.getChainId).toHaveBeenCalledOnce();
	});

	it("does not cache a rejected lookup, so a later call retries", async () => {
		const provider = {
			getChainId: vi.fn().mockRejectedValueOnce(new Error("network error")).mockResolvedValueOnce(7),
		} as unknown as PublicClient;

		await expect(loadChainId(provider)).rejects.toThrow("network error");
		await expect(loadChainId(provider)).resolves.toBe(7);
		expect(provider.getChainId).toHaveBeenCalledTimes(2);
	});
});

const GENESIS_TS = 1_600_000_000;

/** Chain whose block timestamps follow `tsAt`; getBlock resolves any probed number. */
const makeTimestampProvider = (tsAt: (block: number) => number): PublicClient =>
	({
		getBlock: vi.fn(({ blockNumber }: { blockNumber: bigint }) =>
			Promise.resolve({ number: blockNumber, timestamp: BigInt(tsAt(Number(blockNumber))) }),
		),
	}) as unknown as PublicClient;

describe("estimateBlockAt", () => {
	const HEAD = 1_000_000n;

	it("converges on a uniform chain matching the nominal seed", async () => {
		const tsAt = (block: number) => GENESIS_TS + block * 5;
		const provider = makeTimestampProvider(tsAt);
		const { block, driftSeconds } = await estimateBlockAt(provider, tsAt(400_000), {
			number: HEAD,
			timestamp: tsAt(1_000_000),
		});
		expect(block).toBe(400_000n);
		expect(Math.abs(driftSeconds)).toBeLessThanOrEqual(600);
	});

	it("converges when the real cadence is far from the seed", async () => {
		const tsAt = (block: number) => GENESIS_TS + block * 12;
		const provider = makeTimestampProvider(tsAt);
		const { block, driftSeconds } = await estimateBlockAt(provider, tsAt(400_000), {
			number: HEAD,
			timestamp: tsAt(1_000_000),
		});
		expect(Math.abs(driftSeconds)).toBeLessThanOrEqual(600);
		expect(Math.abs(Number(block) - 400_000)).toBeLessThanOrEqual(600 / 12);
	});

	it("converges across a cadence shift for a target in the old regime", async () => {
		// 12s blocks before 500k, 5s after — the target sits in the slow regime.
		const tsAt = (block: number) =>
			block < 500_000 ? GENESIS_TS + block * 12 : GENESIS_TS + 500_000 * 12 + (block - 500_000) * 5;
		const provider = makeTimestampProvider(tsAt);
		const { driftSeconds } = await estimateBlockAt(provider, tsAt(100_000), {
			number: HEAD,
			timestamp: tsAt(1_000_000),
		});
		expect(Math.abs(driftSeconds)).toBeLessThanOrEqual(600);
	});

	it("returns Infinity drift when every probe fails", async () => {
		const provider = { getBlock: vi.fn().mockRejectedValue(new Error("pruned")) } as unknown as PublicClient;
		const { driftSeconds } = await estimateBlockAt(provider, GENESIS_TS, {
			number: HEAD,
			timestamp: GENESIS_TS + 5_000_000,
		});
		expect(driftSeconds).toBe(Number.POSITIVE_INFINITY);
	});

	it("pins a future timestamp to the head", async () => {
		const tsAt = (block: number) => GENESIS_TS + block * 5;
		const provider = makeTimestampProvider(tsAt);
		const { block } = await estimateBlockAt(provider, tsAt(1_000_000) + 100_000, {
			number: HEAD,
			timestamp: tsAt(1_000_000),
		});
		expect(block).toBe(HEAD);
	});
});

describe("getTargetedBlockRange", () => {
	const HEAD_RANGE = { fromBlock: 990_000n, toBlock: 1_000_000n };
	const RANGE = 10_000n;
	const tsAt = (block: number) => GENESIS_TS + block * 5;

	it("aims a window at an old timestamp, disjoint from the head window", async () => {
		const provider = makeTimestampProvider(tsAt);
		const range = await getTargetedBlockRange(provider, RANGE, tsAt(400_000), HEAD_RANGE);
		expect(range).toEqual({ fromBlock: 399_000n, toBlock: 409_000n });
	});

	it("clamps the window just below the head window", async () => {
		const provider = makeTimestampProvider(tsAt);
		const range = await getTargetedBlockRange(provider, RANGE, tsAt(985_000), HEAD_RANGE);
		expect(range).toEqual({ fromBlock: 984_000n, toBlock: 989_999n });
	});

	it("returns null for a recent timestamp already inside the head window", async () => {
		const provider = makeTimestampProvider(tsAt);
		const range = await getTargetedBlockRange(provider, RANGE, tsAt(995_000), HEAD_RANGE);
		expect(range).toBeNull();
	});

	it("returns null when the head window already reaches genesis", async () => {
		const provider = makeTimestampProvider(tsAt);
		const range = await getTargetedBlockRange(provider, RANGE, tsAt(1_000), { fromBlock: 0n, toBlock: 5_000n });
		expect(range).toBeNull();
		expect(provider.getBlock).not.toHaveBeenCalled();
	});

	it("returns null when block probes fail, keeping the caller on the head window", async () => {
		const provider = { getBlock: vi.fn().mockRejectedValue(new Error("pruned")) } as unknown as PublicClient;
		const range = await getTargetedBlockRange(provider, RANGE, tsAt(400_000), HEAD_RANGE);
		expect(range).toBeNull();
	});
});
