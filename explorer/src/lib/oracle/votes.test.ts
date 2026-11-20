import {
	type Address,
	encodeAbiParameters,
	encodeEventTopics,
	getAbiItem,
	type Hex,
	numberToHex,
	type PublicClient,
} from "viem";
import { describe, expect, it, vi } from "vitest";
import { sentinelOracleAbi } from "./abi";
import { loadSentinelVotes } from "./votes";

const ORACLE: Address = "0x1234567890123456789012345678901234567890";
const REQUEST_ID: Hex = "0x8140f61f913a7d3602ddc7d468be9b047c6dec24481ed4ca71f608f13ad46d46";
const SENTINEL_A: Address = "0x1111111111111111111111111111111111111111";
const SENTINEL_B: Address = "0x2222222222222222222222222222222222222222";

const commonParams = {
	oracle: ORACLE,
	requestId: REQUEST_ID,
	proposedAt: 1n,
	revealDeadline: 900n,
	maxBlockRange: 0n,
};

// biome-ignore lint/suspicious/noExplicitAny: viem's ABI input types don't always include the `indexed` property
const nonIndexedInputs = (inputs: readonly any[]) => inputs.filter((i: any) => !i.indexed);

const makeRawVoteLog = ({
	eventName,
	indexedArgs,
	nonIndexedValues,
	blockNumber,
	logIndex = 0,
}: {
	eventName: "Committed" | "Revealed";
	indexedArgs: Record<string, unknown>;
	nonIndexedValues: unknown[];
	blockNumber: bigint;
	logIndex?: number;
}) => {
	const topics = encodeEventTopics({ abi: sentinelOracleAbi, eventName, args: indexedArgs });
	const abiItem = getAbiItem({ abi: sentinelOracleAbi, name: eventName }) as { inputs: readonly unknown[] };
	const data = encodeAbiParameters(nonIndexedInputs(abiItem.inputs), nonIndexedValues);
	return {
		address: ORACLE,
		topics,
		data,
		blockNumber: numberToHex(blockNumber),
		logIndex: numberToHex(logIndex),
		transactionHash: `0x${"00".repeat(32)}`,
		blockHash: `0x${"00".repeat(32)}`,
		transactionIndex: "0x0",
		removed: false,
	};
};

const makeCommittedLog = (sentinel: Address, blockNumber: bigint, logIndex = 0) =>
	makeRawVoteLog({
		eventName: "Committed",
		indexedArgs: { requestId: REQUEST_ID, sentinel },
		nonIndexedValues: [0n],
		blockNumber,
		logIndex,
	});

const makeRevealedLog = (sentinel: Address, approved: boolean, reason: string, blockNumber: bigint, logIndex = 0) =>
	makeRawVoteLog({
		eventName: "Revealed",
		indexedArgs: { requestId: REQUEST_ID, sentinel },
		nonIndexedValues: [approved, 0n, reason],
		blockNumber,
		logIndex,
	});

const makeProvider = (logs: unknown[], latestBlock = 1000n): PublicClient =>
	({
		getBlockNumber: vi.fn().mockResolvedValue(latestBlock),
		request: vi.fn().mockResolvedValue(logs),
	}) as unknown as PublicClient;

describe("loadSentinelVotes", () => {
	it("makes a single eth_getLogs request filtered by the given requestId", async () => {
		const provider = makeProvider([]);

		await loadSentinelVotes({ provider, ...commonParams });

		const requestMock = provider.request as unknown as ReturnType<typeof vi.fn>;
		expect(requestMock).toHaveBeenCalledTimes(1);
		const params = requestMock.mock.calls[0][0].params[0];
		expect(params.address).toBe(ORACLE);
		expect(Array.isArray(params.topics[0])).toBe(true);
		expect(params.topics[0]).toHaveLength(2);
		expect(params.topics[1]).toBe(REQUEST_ID);
	});

	const getLogsRanges = (provider: PublicClient) =>
		(provider.request as unknown as ReturnType<typeof vi.fn>).mock.calls.map(([{ params }]) => [
			params[0].fromBlock,
			params[0].toBlock,
		]);

	it("searches an old request's whole vote window", async () => {
		const provider = makeProvider([], 1_000_000n);

		await loadSentinelVotes({ provider, ...commonParams, proposedAt: 100n, revealDeadline: 150n });

		expect(getLogsRanges(provider)).toEqual([[numberToHex(100n), numberToHex(150n)]]);
	});

	it("caps the vote window at the latest block while a request is still being voted on", async () => {
		const provider = makeProvider([], 1000n);

		await loadSentinelVotes({ provider, ...commonParams, proposedAt: 990n, revealDeadline: 1040n });

		expect(getLogsRanges(provider)).toEqual([[numberToHex(990n), numberToHex(1000n)]]);
	});

	it("splits the vote window into maxBlockRange-sized requests and joins commits and reveals across them", async () => {
		const provider = makeProvider([], 1000n);
		vi.mocked(provider.request)
			.mockResolvedValueOnce([makeCommittedLog(SENTINEL_A, 110n)] as never)
			.mockResolvedValueOnce([makeCommittedLog(SENTINEL_B, 125n)] as never)
			.mockResolvedValueOnce([makeRevealedLog(SENTINEL_A, true, "ok", 145n)] as never);

		const result = await loadSentinelVotes({
			provider,
			...commonParams,
			proposedAt: 100n,
			revealDeadline: 150n,
			maxBlockRange: 20n,
		});

		expect(getLogsRanges(provider)).toEqual([
			[numberToHex(100n), numberToHex(119n)],
			[numberToHex(120n), numberToHex(139n)],
			[numberToHex(140n), numberToHex(150n)],
		]);
		expect(result).toEqual([
			{ sentinel: SENTINEL_A, state: "approved", reason: "ok" },
			{ sentinel: SENTINEL_B, state: "committed" },
		]);
	});

	it("returns an empty array when nobody has voted", async () => {
		const provider = makeProvider([]);

		const result = await loadSentinelVotes({ provider, ...commonParams });

		expect(result).toEqual([]);
	});

	it("marks a sentinel as committed when it hasn't revealed yet", async () => {
		const provider = makeProvider([makeCommittedLog(SENTINEL_A, 1n)]);

		const result = await loadSentinelVotes({ provider, ...commonParams });

		expect(result).toEqual([{ sentinel: SENTINEL_A, state: "committed" }]);
	});

	it("maps a revealed approval with its reason", async () => {
		const provider = makeProvider([
			makeCommittedLog(SENTINEL_A, 1n),
			makeRevealedLog(SENTINEL_A, true, "looks good", 2n),
		]);

		const result = await loadSentinelVotes({ provider, ...commonParams });

		expect(result).toEqual([{ sentinel: SENTINEL_A, state: "approved", reason: "looks good" }]);
	});

	it("maps a revealed denial with its reason", async () => {
		const provider = makeProvider([
			makeCommittedLog(SENTINEL_A, 1n),
			makeRevealedLog(SENTINEL_A, false, "suspicious payload", 2n),
		]);

		const result = await loadSentinelVotes({ provider, ...commonParams });

		expect(result).toEqual([{ sentinel: SENTINEL_A, state: "denied", reason: "suspicious payload" }]);
	});

	it("orders votes by commit position, first vote first", async () => {
		const provider = makeProvider([
			makeCommittedLog(SENTINEL_B, 5n),
			makeCommittedLog(SENTINEL_A, 2n),
			makeRevealedLog(SENTINEL_A, true, "ok", 6n),
			makeRevealedLog(SENTINEL_B, false, "no", 6n, 1),
		]);

		const result = await loadSentinelVotes({ provider, ...commonParams });

		expect(result).toEqual([
			{ sentinel: SENTINEL_A, state: "approved", reason: "ok" },
			{ sentinel: SENTINEL_B, state: "denied", reason: "no" },
		]);
	});
});
