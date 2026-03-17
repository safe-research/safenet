import {
	type AbiEvent,
	type Address,
	encodeAbiParameters,
	encodeEventTopics,
	formatLog,
	getAddress,
	type Hex,
	keccak256,
	parseAbi,
	parseAbiParameters,
	type RpcLog,
	toEventSelector,
	toHex,
	zeroHash,
} from "viem";
import { describe, expect, it, vi } from "vitest";

import { testLogger } from "../__tests__/config.js";
import { computeLogsBloom } from "../utils/bloom.js";
import type { BlockUpdate } from "./blocks.js";
import { type Client, type Config, EventWatcher, type Log } from "./events.js";

const CONFIG = {
	blockPageSize: 5,
	blockAllLogsQueryRetryCount: 0,
	blockSingleQueryRetryCount: 2,
	maxLogsPerQuery: 10,
	fallibleEvents: [],
};

const WATCH = {
	address: ["0x4141414141414141414141414141414141414141", "0x4242424242424242424242424242424242424242"] as Address[],
	events: parseAbi([
		"event Transfer(address indexed from, address indexed to, uint256 amount)",
		"event Approval(address indexed owner, address indexed spender, uint256 amount)",
	]),
} as const;

type TestLog = Log<typeof WATCH.events>;
type TestConfig = Partial<Pick<Config, "blockAllLogsQueryRetryCount" | "fallibleEvents">>;

const setup = ({ blockAllLogsQueryRetryCount, fallibleEvents }: TestConfig = {}) => {
	const getLogs = vi.fn();
	const config = {
		...CONFIG,
		...WATCH,
		logger: testLogger,
		client: {
			getLogs,
		} as unknown as Client,
		blockAllLogsQueryRetryCount: blockAllLogsQueryRetryCount ?? CONFIG.blockAllLogsQueryRetryCount,
		fallibleEvents: fallibleEvents ?? CONFIG.fallibleEvents,
	};

	return {
		config,
		events: new EventWatcher(config),
		mocks: {
			getLogs,
		},
	};
};

const setupOneQueryPerEvent = async (update: BlockUpdate, config: TestConfig = {}) => {
	const { events, mocks } = setup(config);

	events.onBlockUpdate(update);

	mocks.getLogs.mockRejectedValue(new Error("test"));

	if (update.type === "watcher_update_warp_to_block") {
		for (let i = CONFIG.blockPageSize; i !== 1; i = Math.ceil(i / 2)) {
			await expect(events.next()).rejects.toThrow();
		}
	} else if (update.type === "watcher_update_new_block") {
		const totalRetries = CONFIG.blockAllLogsQueryRetryCount + CONFIG.blockSingleQueryRetryCount;
		for (let i = 0; i < totalRetries; i++) {
			await expect(events.next()).rejects.toThrow();
		}
	}

	mocks.getLogs.mockReset();

	return { events, mocks };
};

const query = (q: ({ blockHash: Hex } | { fromBlock: bigint; toBlock: bigint }) & { event?: AbiEvent }) => ({
	strict: true,
	address: WATCH.address,
	...(q.event === undefined ? { events: WATCH.events, event: undefined } : {}),
	...q,
});

const log = (l: Pick<TestLog, "eventName" | "logIndex"> & Partial<Pick<TestLog, "blockNumber">>) => ({
	blockNumber: 0n,
	...l,
});

const rpcLog = ({ logIndex, ...l }: { logIndex: number } & Pick<RpcLog, "address" | "topics" | "data">) =>
	formatLog({
		...l,
		logIndex: toHex(logIndex),
		blockNumber: "0x0",
		transactionHash: zeroHash,
		transactionIndex: "0x0",
		blockHash: zeroHash,
		blockTimestamp: "0x0",
		removed: false,
	});

const BLOOM_ZERO = `0x${"00".repeat(256)}` as const;
const BLOOM_ALL = `0x${"ff".repeat(256)}` as const;

const bloom = (...data: Hex[]) => {
	let result = 0n;
	for (const datum of data) {
		const digest = BigInt(keccak256(datum));
		result |= 1n << ((digest >> 240n) & 0x7ffn);
		result |= 1n << ((digest >> 224n) & 0x7ffn);
		result |= 1n << ((digest >> 208n) & 0x7ffn);
	}
	return `0x${result.toString(16).padStart(512, "0")}` as const;
};

describe("EventWatcher", () => {
	describe("constructor", () => {
		it("initialize a watcher", async () => {
			const { events, mocks } = setup();

			const logs = await events.next();

			expect(mocks.getLogs).toBeCalledTimes(0);
			expect(logs).toBeNull();
		});
	});

	describe("onBlock", () => {
		it("does not process new block updates before the previous is done", async () => {
			for (const update of [
				{ type: "watcher_update_warp_to_block", fromBlock: 123n, toBlock: 130n },
				{ type: "watcher_update_new_block", blockNumber: 0n, blockHash: zeroHash, logsBloom: BLOOM_ZERO },
			] as const) {
				const { events } = setup();

				events.onBlockUpdate(update);

				expect(() => events.onBlockUpdate(update)).toThrow();
			}
		});
	});

	describe("reorgs", () => {
		it("has no effect on the event watcher", async () => {
			const { events, mocks } = setup();

			events.onBlockUpdate({ type: "watcher_update_uncle_block", blockNumber: 42n });
			const logs = await events.next();

			expect(mocks.getLogs).toBeCalledTimes(0);
			expect(logs).toBeNull();
		});
	});

	describe("warping", () => {
		it("should fetch logs in pages", async () => {
			const { events, mocks } = setup();

			events.onBlockUpdate({ type: "watcher_update_warp_to_block", fromBlock: 123n, toBlock: 130n });

			mocks.getLogs.mockResolvedValueOnce([
				log({ eventName: "Transfer", blockNumber: 123n, logIndex: 42 }),
				log({ eventName: "Transfer", blockNumber: 125n, logIndex: 1 }),
				log({ eventName: "Approval", blockNumber: 125n, logIndex: 0 }),
			]);
			mocks.getLogs.mockResolvedValueOnce([]);

			const logs = [await events.next(), await events.next(), await events.next()];

			expect(mocks.getLogs.mock.calls).toEqual([
				[query({ fromBlock: 123n, toBlock: 127n })],
				[query({ fromBlock: 128n, toBlock: 130n })],
			]);
			expect(logs).toEqual([
				// Note that the logs are sorted.
				[
					log({ eventName: "Transfer", blockNumber: 123n, logIndex: 42 }),
					log({ eventName: "Approval", blockNumber: 125n, logIndex: 0 }),
					log({ eventName: "Transfer", blockNumber: 125n, logIndex: 1 }),
				],
				// And empty logs are allowed.
				[],
				// And once warping is complete, we are back to being idle.
				null,
			]);
		});

		it("should error if the max log length is returned", async () => {
			const { events, mocks } = setup();

			events.onBlockUpdate({ type: "watcher_update_warp_to_block", fromBlock: 123n, toBlock: 130n });

			mocks.getLogs.mockResolvedValueOnce(
				Array(CONFIG.maxLogsPerQuery).map((_, i) => log({ eventName: "Approval", blockNumber: 123n, logIndex: i })),
			);

			await expect(events.next()).rejects.toThrow();
		});

		it("should reduce page sizes on failure", async () => {
			const { events, mocks } = setup();

			events.onBlockUpdate({ type: "watcher_update_warp_to_block", fromBlock: 123n, toBlock: 130n });

			mocks.getLogs.mockRejectedValue(new Error("error"));

			await expect(events.next()).rejects.toThrow();
			await expect(events.next()).rejects.toThrow();
			await expect(events.next()).rejects.toThrow();
			await expect(events.next()).rejects.toThrow();

			expect(mocks.getLogs.mock.calls).toEqual([
				[query({ fromBlock: 123n, toBlock: 127n })],
				[query({ fromBlock: 123n, toBlock: 125n })],
				[query({ fromBlock: 123n, toBlock: 124n })],
				// Note on the last attempt that the query is split into one request per event.
				...WATCH.events.map((event) => [query({ fromBlock: 123n, toBlock: 123n, event })]),
			]);
		});

		it("should reset page size once recovered", async () => {
			const { events, mocks } = setup();

			events.onBlockUpdate({ type: "watcher_update_warp_to_block", fromBlock: 123n, toBlock: 130n });

			mocks.getLogs.mockRejectedValueOnce(new Error("error"));
			mocks.getLogs.mockResolvedValue([]);

			await expect(events.next()).rejects.toThrow();
			await events.next();
			await events.next();

			expect(mocks.getLogs.mock.calls).toEqual([
				[query({ fromBlock: 123n, toBlock: 127n })],
				[query({ fromBlock: 123n, toBlock: 125n })],
				[query({ fromBlock: 126n, toBlock: 130n })],
			]);
		});

		it("errors when at least one query fails when splitting requests per event", async () => {
			const { events, mocks } = await setupOneQueryPerEvent({
				type: "watcher_update_warp_to_block",
				fromBlock: 123n,
				toBlock: 130n,
			});

			mocks.getLogs.mockResolvedValueOnce([log({ eventName: "Transfer", blockNumber: 123n, logIndex: 0 })]);
			mocks.getLogs.mockRejectedValueOnce(new Error("test"));

			await expect(events.next()).rejects.toThrow();
		});

		it("errors when at least one query has too many events when splitting requests per event", async () => {
			const { events, mocks } = await setupOneQueryPerEvent({
				type: "watcher_update_warp_to_block",
				fromBlock: 123n,
				toBlock: 130n,
			});

			mocks.getLogs.mockResolvedValueOnce([]);
			mocks.getLogs.mockResolvedValueOnce(
				[...Array(CONFIG.maxLogsPerQuery)].map((_, i) =>
					log({ eventName: "Approval", blockNumber: 123n, logIndex: i }),
				),
			);

			await expect(events.next()).rejects.toThrow();
		});

		it("allows fallible events to be dropped", async () => {
			const { events, mocks } = await setupOneQueryPerEvent(
				{ type: "watcher_update_warp_to_block", fromBlock: 123n, toBlock: 130n },
				{
					fallibleEvents: ["Approval"],
				},
			);

			mocks.getLogs.mockResolvedValueOnce([log({ eventName: "Transfer", blockNumber: 123n, logIndex: 0 })]);
			mocks.getLogs.mockRejectedValueOnce(new Error("test"));

			const logs = await events.next();

			expect(mocks.getLogs.mock.calls).toEqual([
				...WATCH.events.map((event) => [query({ fromBlock: 123n, toBlock: 123n, event })]),
			]);
			expect(logs).toEqual([log({ eventName: "Transfer", blockNumber: 123n, logIndex: 0 })]);
		});

		it("allows fallible events to have more than log limit", async () => {
			const { events, mocks } = await setupOneQueryPerEvent(
				{ type: "watcher_update_warp_to_block", fromBlock: 123n, toBlock: 130n },
				{
					fallibleEvents: ["Approval"],
				},
			);

			mocks.getLogs.mockResolvedValueOnce([log({ eventName: "Transfer", blockNumber: 123n, logIndex: 0 })]);
			mocks.getLogs.mockResolvedValueOnce(
				[...Array(CONFIG.maxLogsPerQuery)].map((_, i) =>
					log({ eventName: "Approval", blockNumber: 123n, logIndex: i + 1 }),
				),
			);

			const logs = await events.next();

			expect(logs).toEqual([
				log({ eventName: "Transfer", blockNumber: 123n, logIndex: 0 }),
				...[...Array(CONFIG.maxLogsPerQuery)].map((_, i) =>
					log({ eventName: "Approval", blockNumber: 123n, logIndex: i + 1 }),
				),
			]);
		});
	});

	describe("blocks", () => {
		it("query a single block", async () => {
			const { events, mocks } = setup();

			events.onBlockUpdate({
				type: "watcher_update_new_block",
				blockNumber: 1337n,
				blockHash: keccak256(toHex("1337")),
				logsBloom: BLOOM_ALL,
			});

			mocks.getLogs.mockResolvedValueOnce([
				log({ eventName: "Transfer", blockNumber: 1337n, logIndex: 1 }),
				log({ eventName: "Approval", blockNumber: 1337n, logIndex: 0 }),
				log({ eventName: "Transfer", blockNumber: 1337n, logIndex: 2 }),
			]);

			const logs = [await events.next(), await events.next()];

			expect(mocks.getLogs.mock.calls).toEqual([[query({ blockHash: keccak256(toHex("1337")) })]]);
			expect(logs).toEqual([
				// Note that logs are sorted.
				[
					log({ eventName: "Approval", blockNumber: 1337n, logIndex: 0 }),
					log({ eventName: "Transfer", blockNumber: 1337n, logIndex: 1 }),
					log({ eventName: "Transfer", blockNumber: 1337n, logIndex: 2 }),
				],
				// And once the new block is complete, we are back to being idle.
				null,
			]);
		});

		it("should locally filter logs when querying all block logs", async () => {
			const { events, mocks } = setup({
				blockAllLogsQueryRetryCount: 1,
			});

			const allLogs = [
				rpcLog({
					logIndex: 0,
					address: WATCH.address[0],
					topics: encodeEventTopics({
						abi: WATCH.events,
						eventName: "Approval",
						args: {
							owner: `0x${"aa".repeat(20)}`,
							spender: `0x${"bb".repeat(20)}`,
						},
					}) as RpcLog["topics"],
					data: encodeAbiParameters(parseAbiParameters("uint256 amount"), [42n]),
				}),
				rpcLog({
					logIndex: 1,
					address: `0x${"fe".repeat(20)}`,
					topics: encodeEventTopics({
						abi: WATCH.events,
						eventName: "Transfer",
						args: {
							from: `0x${"cc".repeat(20)}`,
							to: `0x${"dd".repeat(20)}`,
						},
					}) as RpcLog["topics"],
					data: encodeAbiParameters(parseAbiParameters("uint256 amount"), [100n]),
				}),
				rpcLog({
					logIndex: 2,
					address: WATCH.address[0],
					topics: encodeEventTopics({
						abi: parseAbi(["event Deposit(address indexed owner, uint256 amount)"]),
						eventName: "Deposit",
						args: {
							owner: `0x${"88".repeat(20)}`,
						},
					}) as RpcLog["topics"],
					data: encodeAbiParameters(parseAbiParameters("uint256 amount"), [1000000000000000000n]),
				}),
				rpcLog({
					logIndex: 3,
					address: WATCH.address[1],
					topics: encodeEventTopics({
						abi: WATCH.events,
						eventName: "Transfer",
						args: {
							from: `0x${"ee".repeat(20)}`,
							to: `0x${"ff".repeat(20)}`,
						},
					}) as RpcLog["topics"],
					data: encodeAbiParameters(parseAbiParameters("uint256 amount"), [1337n]),
				}),
			];

			events.onBlockUpdate({
				type: "watcher_update_new_block",
				blockNumber: 1337n,
				blockHash: keccak256(toHex("1337")),
				logsBloom: computeLogsBloom(allLogs),
			});

			mocks.getLogs.mockResolvedValueOnce(allLogs);

			const logs = await events.next();

			expect(mocks.getLogs.mock.calls).toEqual([[{ blockHash: keccak256(toHex("1337")) }]]);
			expect(logs).toEqual([
				{
					...allLogs[0],
					eventName: "Approval",
					args: {
						owner: getAddress(`0x${"aa".repeat(20)}`),
						spender: getAddress(`0x${"bb".repeat(20)}`),
						amount: 42n,
					},
				},
				{
					...allLogs[3],
					eventName: "Transfer",
					args: {
						from: getAddress(`0x${"ee".repeat(20)}`),
						to: getAddress(`0x${"ff".repeat(20)}`),
						amount: 1337n,
					},
				},
			]);
		});

		it("should error if returned logs does not match bloom filter", async () => {
			const { events, mocks } = setup({
				blockAllLogsQueryRetryCount: 3,
			});

			const allLogs = [
				rpcLog({
					logIndex: 0,
					address: WATCH.address[0],
					topics: encodeEventTopics({
						abi: WATCH.events,
						eventName: "Approval",
						args: {
							owner: `0x${"aa".repeat(20)}`,
							spender: `0x${"bb".repeat(20)}`,
						},
					}) as RpcLog["topics"],
					data: encodeAbiParameters(parseAbiParameters("uint256 amount"), [42n]),
				}),
				rpcLog({
					logIndex: 1,
					address: `0x${"fe".repeat(20)}`,
					topics: encodeEventTopics({
						abi: WATCH.events,
						eventName: "Transfer",
						args: {
							from: `0x${"cc".repeat(20)}`,
							to: `0x${"dd".repeat(20)}`,
						},
					}) as RpcLog["topics"],
					data: encodeAbiParameters(parseAbiParameters("uint256 amount"), [100n]),
				}),
			];

			events.onBlockUpdate({
				type: "watcher_update_new_block",
				blockNumber: 1337n,
				blockHash: keccak256(toHex("1337")),
				logsBloom: computeLogsBloom(allLogs),
			});

			for (const someLogs of [[allLogs[0]], [allLogs[1]], []]) {
				mocks.getLogs.mockResolvedValueOnce(someLogs);
				await expect(events.next()).rejects.toThrow();
			}
		});

		it("query blocks if at least one address and event is in the bloom filter", async () => {
			const { events, mocks } = setup();

			events.onBlockUpdate({
				type: "watcher_update_new_block",
				blockNumber: 1337n,
				blockHash: keccak256(toHex("1337")),
				logsBloom: bloom(WATCH.address[0], toEventSelector(WATCH.events[1])),
			});

			mocks.getLogs.mockResolvedValue([]);

			await events.next();

			expect(mocks.getLogs).toBeCalledTimes(1);
		});

		it("skips queries when address/events are not in the logs bloom", async () => {
			for (const logsBloom of [
				BLOOM_ZERO,
				// Bloom filter contains contract addresses, but no events we care about.
				bloom(...WATCH.address),
				// Bloom filter contains events, but not from the contract addresses we care about.
				bloom(...WATCH.events.map(toEventSelector)),
			] as const) {
				const { events, mocks } = setup();

				events.onBlockUpdate({
					type: "watcher_update_new_block",
					blockNumber: 42n,
					blockHash: keccak256(toHex("the answer to life, the universe, and everything")),
					logsBloom,
				});

				const logs = await events.next();

				expect(mocks.getLogs).toBeCalledTimes(0);
				expect(logs).toEqual([]);
			}
		});

		it("should error if the max log length is returned", async () => {
			const { events, mocks } = setup();

			events.onBlockUpdate({
				type: "watcher_update_new_block",
				blockNumber: 42n,
				blockHash: keccak256(toHex("the answer to life, the universe, and everything")),
				logsBloom: BLOOM_ALL,
			});

			mocks.getLogs.mockResolvedValueOnce(
				[...Array(CONFIG.maxLogsPerQuery)].map((_, i) => log({ eventName: "Approval", blockNumber: 42n, logIndex: i })),
			);

			await expect(events.next()).rejects.toThrow();
		});

		it("falls back to multiple requests per query", async () => {
			const { config, events, mocks } = setup({ blockAllLogsQueryRetryCount: 3 });

			events.onBlockUpdate({
				type: "watcher_update_new_block",
				blockNumber: 1337n,
				blockHash: keccak256(toHex("1337")),
				logsBloom: BLOOM_ALL,
			});

			mocks.getLogs.mockRejectedValue(new Error("test"));

			const totalRetries = config.blockAllLogsQueryRetryCount + config.blockSingleQueryRetryCount;
			for (let i = 0; i < totalRetries; i++) {
				await expect(events.next()).rejects.toThrow();
			}
			await expect(events.next()).rejects.toThrow();

			expect(mocks.getLogs.mock.calls).toEqual([
				...[...Array(config.blockAllLogsQueryRetryCount)].map(() => [{ blockHash: keccak256(toHex("1337")) }]),
				...[...Array(config.blockSingleQueryRetryCount)].map(() => [query({ blockHash: keccak256(toHex("1337")) })]),
				// Note on the last attempt that the query is split into one request per event.
				...WATCH.events.map((event) => [query({ blockHash: keccak256(toHex("1337")), event })]),
			]);
		});

		it("skips fallback event queries that are not in the bloom filter", async () => {
			const { events, mocks } = await setupOneQueryPerEvent({
				type: "watcher_update_new_block",
				blockNumber: 1337n,
				blockHash: keccak256(toHex("1337")),
				logsBloom: bloom(...WATCH.address, toEventSelector(WATCH.events[1])),
			});

			mocks.getLogs.mockRejectedValueOnce(new Error("test"));

			await expect(events.next()).rejects.toThrow();

			// Note that only a query for the `Allowance` event is attempted, as the `Transfer`
			// event is not in the bloom filter.
			expect(mocks.getLogs.mock.calls).toEqual([
				[query({ blockHash: keccak256(toHex("1337")), event: WATCH.events[1] })],
			]);
		});

		it("errors when at least one query fails when splitting requests per event", async () => {
			const { events, mocks } = await setupOneQueryPerEvent({
				type: "watcher_update_new_block",
				blockNumber: 1337n,
				blockHash: keccak256(toHex("1337")),
				logsBloom: BLOOM_ALL,
			});

			mocks.getLogs.mockResolvedValueOnce([log({ eventName: "Transfer", blockNumber: 1337n, logIndex: 0 })]);
			mocks.getLogs.mockRejectedValueOnce(new Error("test"));

			await expect(events.next()).rejects.toThrow();
		});

		it("errors when at least one query has too many events when splitting requests per event", async () => {
			const { events, mocks } = await setupOneQueryPerEvent({
				type: "watcher_update_new_block",
				blockNumber: 1337n,
				blockHash: keccak256(toHex("1337")),
				logsBloom: BLOOM_ALL,
			});

			mocks.getLogs.mockResolvedValueOnce(
				[...Array(CONFIG.maxLogsPerQuery)].map((_, i) =>
					log({ eventName: "Approval", blockNumber: 1337n, logIndex: i }),
				),
			);
			mocks.getLogs.mockResolvedValueOnce([]);

			await expect(events.next()).rejects.toThrow();
		});

		it("allows fallible events to be dropped", async () => {
			const { events, mocks } = await setupOneQueryPerEvent(
				{
					type: "watcher_update_new_block",
					blockNumber: 1337n,
					blockHash: keccak256(toHex("1337")),
					logsBloom: BLOOM_ALL,
				},
				{
					fallibleEvents: ["Transfer"],
				},
			);

			mocks.getLogs.mockRejectedValueOnce(new Error("test"));
			mocks.getLogs.mockResolvedValueOnce([log({ eventName: "Approval", blockNumber: 1337n, logIndex: 0 })]);

			const logs = await events.next();

			expect(mocks.getLogs.mock.calls).toEqual([
				...WATCH.events.map((event) => [query({ blockHash: keccak256(toHex("1337")), event })]),
			]);
			expect(logs).toEqual([log({ eventName: "Approval", blockNumber: 1337n, logIndex: 0 })]);
		});

		it("allows fallible events to have more than log limit", async () => {
			const { events, mocks } = await setupOneQueryPerEvent(
				{
					type: "watcher_update_new_block",
					blockNumber: 1337n,
					blockHash: keccak256(toHex("1337")),
					logsBloom: BLOOM_ALL,
				},
				{
					fallibleEvents: ["Approval"],
				},
			);

			mocks.getLogs.mockResolvedValueOnce([log({ eventName: "Transfer", blockNumber: 1337n, logIndex: 11 })]);
			mocks.getLogs.mockResolvedValueOnce(
				[...Array(CONFIG.maxLogsPerQuery)].map((_, i) =>
					log({ eventName: "Approval", blockNumber: 1337n, logIndex: i }),
				),
			);

			const logs = await events.next();

			expect(logs).toEqual([
				...[...Array(CONFIG.maxLogsPerQuery)].map((_, i) =>
					log({ eventName: "Approval", blockNumber: 1337n, logIndex: i }),
				),
				log({ eventName: "Transfer", blockNumber: 1337n, logIndex: 11 }),
			]);
		});
	});
});
