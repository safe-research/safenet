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
import { oracleRequestId } from "./hashing";
import { loadSentinelVotes } from "./votes";

const ORACLE: Address = "0x1234567890123456789012345678901234567890";
const CONSENSUS: Address = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
const EPOCH = 1n;
const SAFE_TX_HASH: Hex = "0xfe8b85e8d090b16fe8f142d3c9292dc1fc77daf9eb4af8f7cf4a7707d95f4028";
const CHAIN_ID = 1;
const REQUEST_ID: Hex = oracleRequestId({
	chainId: CHAIN_ID,
	consensus: CONSENSUS,
	epoch: EPOCH,
	oracle: ORACLE,
	safeTxHash: SAFE_TX_HASH,
	oracleData: "0x",
});
const SENTINEL_A: Address = "0x1111111111111111111111111111111111111111";
const SENTINEL_B: Address = "0x2222222222222222222222222222222222222222";

const commonParams = {
	oracle: ORACLE,
	consensus: CONSENSUS,
	epoch: EPOCH,
	safeTxHash: SAFE_TX_HASH,
	oracleData: "0x" as Hex,
	maxBlockRange: 500n,
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

const makeProvider = (logs: unknown[]): PublicClient =>
	({
		getBlockNumber: vi.fn().mockResolvedValue(1000n),
		request: vi.fn().mockResolvedValue(logs),
		getChainId: vi.fn().mockResolvedValue(CHAIN_ID),
	}) as unknown as PublicClient;

describe("loadSentinelVotes", () => {
	it("makes a single eth_getLogs request filtered by the derived requestId", async () => {
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
