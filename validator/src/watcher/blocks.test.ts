import { type Block, BlockNotFoundError, type GetBlockParameters, keccak256, numberToHex, toHex } from "viem";
import { describe, expect, it, vi } from "vitest";

import { BlockWatcher, type Client, type Timer } from "./blocks.js";

const CONFIG = {
	blockTime: 2000,
	maxReorgDepth: 2,
	blockPropagationDelay: 500,
	blockRetryDelays: [200, 100, 50],
};

const setupCreate = (config: { lastIndexedBlock: bigint | null; maxReorgDepth?: number }) => {
	const getBlock = vi.fn();
	const now = vi.fn();
	const sleep = vi.fn();

	return {
		create() {
			return BlockWatcher.create({
				...CONFIG,
				...config,
				client: {
					getBlock,
				} as unknown as Client,
				timer: {
					now,
					sleep,
				} as unknown as Timer,
			});
		},
		mocks: {
			getBlock,
			now,
			sleep,
		},
	};
};

const setupNext = async (config: { latestBlock: bigint; startTime: number; maxReorgDepth?: number }) => {
	const { create, mocks } = setupCreate({
		lastIndexedBlock: null,
		...config,
	});

	let created: BlockWatcher | null = null;
	await mocks.getBlock.withImplementation(
		({ blockTag, blockNumber }: GetBlockParameters) => {
			if (blockTag === "latest") {
				return block({ number: 1000n });
			}
			if (blockNumber !== undefined) {
				return block({ number: blockNumber });
			}
			throw new Error("unexpected eth_getBlock parameters");
		},
		async () => {
			created = await create();
		},
	);
	mocks.getBlock.mockClear();

	// We have to trick the type-system here, it doesn't understand that the callback in the
	// `withImplementation` call can change the value and thinks `created: null`.
	const blocks = created as unknown as BlockWatcher;

	// Skip the queued records.
	blocks.queued();

	// Setup useful default mocks for the timer.
	let time = config.startTime;
	mocks.now.mockImplementation(() => time);
	mocks.sleep.mockImplementation((ms: number) => {
		time += ms;
		return Promise.resolve();
	});

	return {
		next() {
			return blocks.next();
		},
		mocks,
	};
};

const block = (
	b: {
		number: bigint;
	} & Partial<Pick<Block<bigint, false, "latest">, "hash" | "timestamp" | "parentHash" | "logsBloom">>,
) => ({
	number: b.number,
	hash: b.hash ?? numberToHex(b.number, { size: 32 }),
	timestamp: b.timestamp ?? (b.number * BigInt(CONFIG.blockTime)) / 1000n,
	parentHash: b.parentHash ?? numberToHex(b.number - 1n, { size: 32 }),
	logsBloom: b.logsBloom ?? numberToHex(b.number, { size: 512 }),
});

const newBlockRecord = (
	b: {
		number: bigint;
	} & Partial<Pick<Block<bigint, false, "latest">, "hash" | "logsBloom">>,
) => ({
	type: "record_new_block",
	blockNumber: b.number,
	blockHash: b.hash ?? numberToHex(b.number, { size: 32 }),
	logsBloom: b.logsBloom ?? numberToHex(b.number, { size: 512 }),
});

describe("BlockWatcher", () => {
	describe("create", () => {
		it("initialize a watcher", async () => {
			const { create, mocks } = setupCreate({ lastIndexedBlock: null });

			mocks.getBlock.mockReturnValueOnce(block({ number: 1000n }));
			mocks.getBlock.mockReturnValueOnce(block({ number: 999n }));

			const blocks = await create();
			expect(mocks.getBlock.mock.calls).toEqual([[{ blockTag: "latest" }], [{ blockNumber: 999n }]]);

			expect(blocks.queued()).toStrictEqual([newBlockRecord({ number: 999n }), newBlockRecord({ number: 1000n })]);
		});

		it("should support continuing from last indexed block", async () => {
			const { create, mocks } = setupCreate({ lastIndexedBlock: 900n });

			mocks.getBlock.mockReturnValueOnce(block({ number: 1000n }));
			mocks.getBlock.mockReturnValueOnce(block({ number: 999n }));

			const blocks = await create();
			expect(mocks.getBlock.mock.calls).toEqual([[{ blockTag: "latest" }], [{ blockNumber: 999n }]]);

			expect(blocks.queued()).toStrictEqual([
				{
					type: "record_uncle_block",
					// Uncling 899, means that we go back to block 898 after starting on 900, which makes
					// sense given our 2 max reorg depth. I hope there is no off-by-one error!
					blockNumber: 899n,
				},
				{
					type: "record_warp_to_block",
					fromBlock: 899n,
					toBlock: 998n,
				},
				newBlockRecord({ number: 999n }),
				newBlockRecord({ number: 1000n }),
			]);
		});

		it("should support no reorg protection", async () => {
			const { create, mocks } = setupCreate({ lastIndexedBlock: 900n, maxReorgDepth: 0 });

			mocks.getBlock.mockResolvedValueOnce(block({ number: 1000n }));

			const blocks = await create();
			expect(mocks.getBlock.mock.calls).toEqual([[{ blockTag: "latest" }]]);

			expect(blocks.queued()).toStrictEqual([
				{
					type: "record_warp_to_block",
					fromBlock: 901n,
					toBlock: 1000n,
				},
			]);
		});

		it("should handle reorgs during initialization", async () => {
			const { create, mocks } = setupCreate({ lastIndexedBlock: null, maxReorgDepth: 3 });

			mocks.getBlock.mockReturnValueOnce(
				block({ number: 1000n, hash: keccak256(toHex("bad1000")), parentHash: keccak256(toHex("bad999")) }),
			);
			mocks.getBlock.mockReturnValueOnce(block({ number: 998n }));
			mocks.getBlock.mockReturnValueOnce(
				block({ number: 999n, hash: keccak256(toHex("bad999")), parentHash: keccak256(toHex("uncle")) }),
			);
			mocks.getBlock.mockReturnValueOnce(block({ number: 998n }));
			mocks.getBlock.mockReturnValueOnce(block({ number: 999n }));
			mocks.getBlock.mockReturnValueOnce(block({ number: 1000n }));

			const blocks = await create();
			expect(mocks.getBlock.mock.calls).toEqual([
				[{ blockTag: "latest" }],
				[{ blockNumber: 998n }],
				[{ blockNumber: 999n }],
				[{ blockNumber: 998n }],
				[{ blockNumber: 999n }],
				[{ blockNumber: 1000n }],
			]);

			expect(blocks.queued()).toStrictEqual([
				newBlockRecord({ number: 998n }),
				newBlockRecord({ number: 999n }),
				newBlockRecord({ number: 1000n }),
			]);
		});
	});

	describe("next", () => {
		it("wait for the pending block and fetch it", async () => {
			const { next, mocks } = await setupNext({ latestBlock: 1000n, startTime: 2000100 });

			mocks.getBlock.mockResolvedValueOnce(block({ number: 1001n }));

			const record = await next();
			expect(mocks.sleep.mock.calls).toEqual([[1900 + 500]]);
			expect(mocks.getBlock.mock.calls).toEqual([[{ blockNumber: 1001n }]]);

			expect(record).toStrictEqual(newBlockRecord({ number: 1001n }));
		});

		it("retries if block is not ready when expected", async () => {
			const { next, mocks } = await setupNext({ latestBlock: 1000n, startTime: 2000100 });

			mocks.getBlock.mockRejectedValueOnce(new BlockNotFoundError({ blockNumber: 1001n }));
			mocks.getBlock.mockRejectedValueOnce(new BlockNotFoundError({ blockNumber: 1001n }));
			mocks.getBlock.mockResolvedValueOnce(block({ number: 1001n }));

			const record = await next();
			expect(mocks.sleep.mock.calls).toEqual([[1900 + 500], [200], [100]]);
			expect(mocks.getBlock.mock.calls).toEqual([
				[{ blockNumber: 1001n }],
				[{ blockNumber: 1001n }],
				[{ blockNumber: 1001n }],
			]);

			expect(record).toStrictEqual(newBlockRecord({ number: 1001n }));
		});

		it("skips slots", async () => {
			const { next, mocks } = await setupNext({ latestBlock: 1000n, startTime: 2000100 });

			mocks.getBlock.mockRejectedValueOnce(new BlockNotFoundError({ blockNumber: 1001n }));
			mocks.getBlock.mockRejectedValueOnce(new BlockNotFoundError({ blockNumber: 1001n }));
			mocks.getBlock.mockRejectedValueOnce(new BlockNotFoundError({ blockNumber: 1001n }));
			mocks.getBlock.mockRejectedValueOnce(new BlockNotFoundError({ blockNumber: 1001n }));
			mocks.getBlock.mockResolvedValueOnce(block({ number: 1001n }));

			const record = await next();
			expect(mocks.sleep.mock.calls).toEqual([[1900 + 500], [200], [100], [50], [1650]]);
			expect(mocks.getBlock.mock.calls).toEqual([
				[{ blockNumber: 1001n }],
				[{ blockNumber: 1001n }],
				[{ blockNumber: 1001n }],
				[{ blockNumber: 1001n }],
				[{ blockNumber: 1001n }],
			]);

			expect(record).toStrictEqual(newBlockRecord({ number: 1001n }));
		});

		it("supports deep reorgs", async () => {
			const { next, mocks } = await setupNext({ latestBlock: 1000n, startTime: 2000100, maxReorgDepth: 5 });

			mocks.getBlock.mockResolvedValueOnce(
				block({ number: 1001n, hash: keccak256(toHex("reorg1001")), parentHash: keccak256(toHex("reorg1000")) }),
			);
			mocks.getBlock.mockResolvedValueOnce(
				block({ number: 1000n, hash: keccak256(toHex("reorg1000")), parentHash: keccak256(toHex("reorg999")) }),
			);
			mocks.getBlock.mockResolvedValueOnce(
				block({ number: 999n, hash: keccak256(toHex("reorg999")), parentHash: keccak256(toHex("reorg998")) }),
			);
			mocks.getBlock.mockResolvedValueOnce(block({ number: 998n, hash: keccak256(toHex("reorg998")) }));
			mocks.getBlock.mockResolvedValueOnce(
				block({ number: 999n, hash: keccak256(toHex("reorg999")), parentHash: keccak256(toHex("reorg998")) }),
			);

			const records = [await next(), await next(), await next(), await next(), await next()];
			expect(mocks.sleep.mock.calls).toEqual([[1900 + 500]]);
			expect(mocks.getBlock.mock.calls).toEqual([
				[{ blockNumber: 1001n }],
				[{ blockNumber: 1000n }],
				[{ blockNumber: 999n }],
				[{ blockNumber: 998n }],
				[{ blockNumber: 999n }],
			]);

			expect(records).toStrictEqual([
				{ type: "record_uncle_block", blockNumber: 1000n },
				{ type: "record_uncle_block", blockNumber: 999n },
				{ type: "record_uncle_block", blockNumber: 998n },
				newBlockRecord({ number: 998n, hash: keccak256(toHex("reorg998")) }),
				newBlockRecord({ number: 999n, hash: keccak256(toHex("reorg999")) }),
			]);
		});
	});
});
