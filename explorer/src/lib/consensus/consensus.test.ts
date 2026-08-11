import type { Address, Hex, PublicClient } from "viem";
import { encodeAbiParameters, encodeEventTopics, getAbiItem, numberToHex } from "viem";
import { describe, expect, it, vi } from "vitest";
import { consensusAbi } from "./abi";
import { loadEpochRolloverHistory, loadEpochsState } from "./epochs";
import { loadProposedSafeTransaction, loadTransactionProposals } from "./transactions";

const CONSENSUS = "0x0000000000000000000000000000000000000001" as Address;
const SAFE_TX_HASH = `0x${"ab".repeat(32)}` as Hex;
const SAFE_ADDRESS = "0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF" as Address;
const CURRENT_BLOCK = 10000n;
const MAX_BLOCK_RANGE = 1000n;
const SIGNING_TIMEOUT = 12;

const GROUP_ID_A = `0x${"aa".repeat(32)}` as Hex;
const GROUP_ID_B = `0x${"bb".repeat(32)}` as Hex;

const makeProvider = (): PublicClient =>
	({
		getBlockNumber: vi.fn().mockResolvedValue(CURRENT_BLOCK),
		request: vi.fn().mockResolvedValue([]),
	}) as unknown as PublicClient;

const firstCall = (provider: PublicClient) => (provider.request as ReturnType<typeof vi.fn>).mock.calls[0][0].params[0];

describe("loadTransactionProposals", () => {
	describe("topic filters", () => {
		it("makes a single eth_getLogs request", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				maxBlockRange: MAX_BLOCK_RANGE,
				signingTimeout: SIGNING_TIMEOUT,
			});
			expect((provider.request as ReturnType<typeof vi.fn>).mock.calls).toHaveLength(1);
		});

		it("passes both event selectors as an OR filter in topic[0]", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				maxBlockRange: MAX_BLOCK_RANGE,
				signingTimeout: SIGNING_TIMEOUT,
			});
			expect(Array.isArray(firstCall(provider).topics[0])).toBe(true);
			expect(firstCall(provider).topics[0]).toHaveLength(2);
		});

		it("uses null for topic[1] when safeTxHash is not provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				maxBlockRange: MAX_BLOCK_RANGE,
				signingTimeout: SIGNING_TIMEOUT,
			});
			expect(firstCall(provider).topics[1]).toBeNull();
		});

		it("filters by safeTxHash in topic[1] when provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safeTxHash: SAFE_TX_HASH,
				maxBlockRange: MAX_BLOCK_RANGE,
				signingTimeout: SIGNING_TIMEOUT,
			});
			expect(firstCall(provider).topics[1]).toBe(SAFE_TX_HASH);
		});

		it("uses null for topic[3] when safe is not provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				maxBlockRange: MAX_BLOCK_RANGE,
				signingTimeout: SIGNING_TIMEOUT,
			});
			expect(firstCall(provider).topics[3]).toBeNull();
		});

		it("filters by safe address in topic[3] when provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safe: SAFE_ADDRESS,
				maxBlockRange: MAX_BLOCK_RANGE,
				signingTimeout: SIGNING_TIMEOUT,
			});
			expect(firstCall(provider).topics[2]).toBeNull(); // chainId wildcard
			expect(firstCall(provider).topics[3]).toBe("0x000000000000000000000000DeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF");
		});
	});

	describe("block range", () => {
		it("always includes an explicit toBlock in the request", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				maxBlockRange: MAX_BLOCK_RANGE,
				signingTimeout: SIGNING_TIMEOUT,
			});
			expect(firstCall(provider).toBlock).toBe(numberToHex(CURRENT_BLOCK));
		});

		it("does not call getBlockNumber when toBlock is provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				toBlock: 6000n,
				maxBlockRange: MAX_BLOCK_RANGE,
				signingTimeout: SIGNING_TIMEOUT,
			});
			expect(provider.getBlockNumber).not.toHaveBeenCalled();
		});
	});

	describe("return value", () => {
		it("returns the fromBlock and toBlock used for the query", async () => {
			const provider = makeProvider();
			const result = await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				toBlock: 6000n,
				maxBlockRange: MAX_BLOCK_RANGE,
				signingTimeout: SIGNING_TIMEOUT,
			});
			expect(result.fromBlock).toBe(5000n);
			expect(result.toBlock).toBe(6000n);
			expect(result.proposals).toEqual([]);
		});
	});
});

// biome-ignore lint/suspicious/noExplicitAny: viem's ABI input types don't always include the `indexed` property
const nonIndexedInputs = (inputs: readonly any[]) => inputs.filter((i: any) => !i.indexed);

const ORACLE_TX = {
	chainId: 1n,
	safe: SAFE_ADDRESS,
	to: SAFE_ADDRESS,
	value: 0n,
	data: "0x" as Hex,
	operation: 0,
	safeTxGas: 0n,
	baseGas: 0n,
	gasPrice: 0n,
	gasToken: "0x0000000000000000000000000000000000000000" as Address,
	refundReceiver: "0x0000000000000000000000000000000000000000" as Address,
	nonce: 0n,
};

const makeRawConsensusLog = ({
	eventName,
	indexedArgs,
	nonIndexedValues,
	blockNumber,
	logIndex = 0,
}: {
	eventName: "TransactionProposed" | "TransactionAttested";
	indexedArgs: Record<string, unknown>;
	nonIndexedValues: unknown[];
	blockNumber: bigint;
	logIndex?: number;
}) => {
	const topics = encodeEventTopics({ abi: consensusAbi, eventName, args: indexedArgs });
	const abiItem = getAbiItem({ abi: consensusAbi, name: eventName }) as { inputs: readonly unknown[] };
	const data = encodeAbiParameters(nonIndexedInputs(abiItem.inputs), nonIndexedValues);
	return {
		address: CONSENSUS,
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

const makeOracleProposedLog = ({
	safeTxHash,
	epoch,
	oracle,
	blockNumber,
}: {
	safeTxHash: Hex;
	epoch: bigint;
	oracle: Address;
	blockNumber: bigint;
}) =>
	makeRawConsensusLog({
		eventName: "TransactionProposed",
		indexedArgs: { safeTxHash, chainId: 1n, safe: SAFE_ADDRESS },
		nonIndexedValues: [epoch, oracle, ORACLE_TX],
		blockNumber,
	});

const makeOracleAttestedLog = ({
	safeTxHash,
	epoch,
	oracle,
	blockNumber,
	logIndex = 1,
}: {
	safeTxHash: Hex;
	epoch: bigint;
	oracle: Address;
	blockNumber: bigint;
	logIndex?: number;
}) =>
	makeRawConsensusLog({
		eventName: "TransactionAttested",
		indexedArgs: { safeTxHash, chainId: 1n, safe: SAFE_ADDRESS },
		nonIndexedValues: [epoch, oracle, `0x${"00".repeat(32)}`, { r: { x: 0n, y: 0n }, z: 0n }],
		blockNumber,
		logIndex,
	});

const makeProviderWithLogs = (logs: ReturnType<typeof makeRawConsensusLog>[]): PublicClient =>
	({
		getBlockNumber: vi.fn().mockResolvedValue(CURRENT_BLOCK),
		request: vi.fn().mockResolvedValue(logs),
	}) as unknown as PublicClient;

describe("loadProposedSafeTransaction", () => {
	it("topic filters", async () => {
		const provider = makeProviderWithLogs([]);
		await loadProposedSafeTransaction({
			provider,
			consensus: CONSENSUS,
			safeTxHash: SAFE_TX_HASH,
			maxBlockRange: MAX_BLOCK_RANGE,
		});
		expect(Array.isArray(firstCall(provider).topics[0])).toBe(true);
		expect(firstCall(provider).topics[0]).toHaveLength(1);
		expect(firstCall(provider).topics[1]).toBe(SAFE_TX_HASH);
	});

	it("returns the transaction from a TransactionProposed log", async () => {
		const oracle = "0x3333333333333333333333333333333333333333" as Address;
		const provider = makeProviderWithLogs([
			makeOracleProposedLog({ safeTxHash: SAFE_TX_HASH, epoch: 1n, oracle, blockNumber: 100n }),
		]);
		const result = await loadProposedSafeTransaction({
			provider,
			consensus: CONSENSUS,
			safeTxHash: SAFE_TX_HASH,
			maxBlockRange: MAX_BLOCK_RANGE,
		});
		expect(result).toEqual(ORACLE_TX);
	});

	it("returns null when no proposal log is found", async () => {
		const provider = makeProviderWithLogs([]);
		const result = await loadProposedSafeTransaction({
			provider,
			consensus: CONSENSUS,
			safeTxHash: SAFE_TX_HASH,
			maxBlockRange: MAX_BLOCK_RANGE,
		});
		expect(result).toBeNull();
	});
});

describe("loadTransactionProposals oracle recognition", () => {
	const ORACLE = "0x3333333333333333333333333333333333333333" as Address;
	const OTHER_ORACLE = "0x4444444444444444444444444444444444444444" as Address;

	it("keeps and tags an oracle proposal whose oracle is in the allow-list", async () => {
		const provider = makeProviderWithLogs([
			makeOracleProposedLog({ safeTxHash: SAFE_TX_HASH, epoch: 1n, oracle: ORACLE, blockNumber: CURRENT_BLOCK }),
			makeOracleProposedLog({ safeTxHash: SAFE_TX_HASH, epoch: 1n, oracle: OTHER_ORACLE, blockNumber: CURRENT_BLOCK }),
		]);
		const result = await loadTransactionProposals({
			provider,
			consensus: CONSENSUS,
			maxBlockRange: MAX_BLOCK_RANGE,
			signingTimeout: SIGNING_TIMEOUT,
			oracles: [ORACLE],
		});
		expect(result.proposals).toHaveLength(1);
		expect(result.proposals[0].oracle).toBe(ORACLE);
	});

	it("derives trust from a TransactionAttested log when no allow-list is configured", async () => {
		const provider = makeProviderWithLogs([
			makeOracleProposedLog({ safeTxHash: SAFE_TX_HASH, epoch: 1n, oracle: OTHER_ORACLE, blockNumber: CURRENT_BLOCK }),
			makeOracleProposedLog({ safeTxHash: SAFE_TX_HASH, epoch: 1n, oracle: ORACLE, blockNumber: CURRENT_BLOCK }),
			makeOracleAttestedLog({ safeTxHash: SAFE_TX_HASH, epoch: 1n, oracle: ORACLE, blockNumber: CURRENT_BLOCK }),
		]);
		const result = await loadTransactionProposals({
			provider,
			consensus: CONSENSUS,
			maxBlockRange: MAX_BLOCK_RANGE,
			signingTimeout: SIGNING_TIMEOUT,
		});
		expect(result.proposals).toHaveLength(1);
		expect(result.proposals[0].oracle).toBe(ORACLE);
		expect(result.proposals[0].status).toBe("ATTESTED");
	});
});

const makeStagedLog = ({
	activeEpoch,
	proposedEpoch,
	rolloverBlock,
	groupId,
	blockNumber,
	logIndex = 0,
}: {
	activeEpoch: bigint;
	proposedEpoch: bigint;
	rolloverBlock: bigint;
	groupId: Hex;
	blockNumber: bigint;
	logIndex?: number;
}) => ({
	args: {
		activeEpoch,
		proposedEpoch,
		rolloverBlock,
		groupId,
		groupKey: { x: 1n, y: 2n },
		signatureId: `0x${"00".repeat(32)}`,
		attestation: { r: { x: 0n, y: 0n }, z: 0n },
	},
	blockNumber,
	logIndex,
	transactionHash: `0x${"00".repeat(32)}`,
	blockHash: `0x${"00".repeat(32)}`,
	address: CONSENSUS,
	data: "0x",
	topics: [],
	transactionIndex: 0,
	removed: false,
});

describe("loadEpochRolloverHistory", () => {
	const makeEpochProvider = ({
		blockNumber = 1000n,
		logs = [],
	}: {
		blockNumber?: bigint;
		logs?: ReturnType<typeof makeStagedLog>[];
	} = {}): PublicClient =>
		({
			getBlockNumber: vi.fn().mockResolvedValue(blockNumber),
			getLogs: vi.fn().mockResolvedValue(logs),
		}) as unknown as PublicClient;

	it("returns empty entries when no logs are found", async () => {
		const result = await loadEpochRolloverHistory({
			provider: makeEpochProvider({ blockNumber: 1000n }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.entries).toEqual([]);
		expect(result.reachedGenesis).toBe(false);
		expect(result.fromBlock).toBe(500n);
	});

	it("returns reachedGenesis true when fromBlock reaches 0", async () => {
		// blockNumber < maxBlockRange → fromBlock clamps to 0
		const result = await loadEpochRolloverHistory({
			provider: makeEpochProvider({ blockNumber: 100n }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.entries).toEqual([]);
		expect(result.reachedGenesis).toBe(true);
		expect(result.fromBlock).toBe(0n);
	});

	it("maps EpochStaged logs to rollover entries", async () => {
		const logs = [
			makeStagedLog({
				activeEpoch: 1n,
				proposedEpoch: 2n,
				rolloverBlock: 150n,
				groupId: GROUP_ID_A,
				blockNumber: 200n,
			}),
		];
		const result = await loadEpochRolloverHistory({
			provider: makeEpochProvider({ logs }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.entries).toHaveLength(1);
		expect(result.entries[0]).toEqual({
			activeEpoch: 1n,
			proposedEpoch: 2n,
			rolloverBlock: 150n,
			groupId: GROUP_ID_A,
			stagedAt: 200n,
		});
	});

	it("detects genesis when activeEpoch is 0", async () => {
		const logs = [
			makeStagedLog({
				activeEpoch: 0n,
				proposedEpoch: 1n,
				rolloverBlock: 10n,
				groupId: GROUP_ID_A,
				blockNumber: 50n,
			}),
		];
		const result = await loadEpochRolloverHistory({
			provider: makeEpochProvider({ logs }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.reachedGenesis).toBe(true);
	});

	it("does not detect genesis when only proposedEpoch is 0", async () => {
		const logs = [
			makeStagedLog({
				activeEpoch: 1n,
				proposedEpoch: 0n,
				rolloverBlock: 10n,
				groupId: GROUP_ID_A,
				blockNumber: 50n,
			}),
		];
		const result = await loadEpochRolloverHistory({
			provider: makeEpochProvider({ logs }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.reachedGenesis).toBe(false);
	});

	it("returns entries sorted most recent first", async () => {
		const logs = [
			makeStagedLog({
				activeEpoch: 1n,
				proposedEpoch: 2n,
				rolloverBlock: 100n,
				groupId: GROUP_ID_A,
				blockNumber: 100n,
			}),
			makeStagedLog({
				activeEpoch: 2n,
				proposedEpoch: 3n,
				rolloverBlock: 200n,
				groupId: GROUP_ID_B,
				blockNumber: 200n,
			}),
		];
		const result = await loadEpochRolloverHistory({
			provider: makeEpochProvider({ logs }),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.entries).toHaveLength(2);
		expect(result.entries[0].proposedEpoch).toBe(3n);
		expect(result.entries[1].proposedEpoch).toBe(2n);
	});

	it("uses cursor - 1 as toBlock when provided to avoid duplicating the boundary entry", async () => {
		const provider = makeEpochProvider();
		await loadEpochRolloverHistory({
			provider,
			consensus: CONSENSUS,
			maxBlockRange: 500n,
			cursor: 800n,
		});
		expect(provider.getLogs).toHaveBeenCalledWith(
			expect.objectContaining({
				toBlock: 799n,
			}),
		);
		expect(provider.getBlockNumber).not.toHaveBeenCalled();
	});

	it("exposes fromBlock in the result for pagination cursor fallback", async () => {
		const provider = makeEpochProvider({ blockNumber: 1000n });
		const result = await loadEpochRolloverHistory({
			provider,
			consensus: CONSENSUS,
			maxBlockRange: 300n,
			cursor: 800n,
		});
		// cursor - 1 = 799, fromBlock = 799 - 300 = 499
		expect(result.fromBlock).toBe(499n);
	});

	it("uses current block number as toBlock when no cursor is provided", async () => {
		const provider = makeEpochProvider({ blockNumber: 1000n });
		await loadEpochRolloverHistory({
			provider,
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(provider.getLogs).toHaveBeenCalledWith(
			expect.objectContaining({
				toBlock: 1000n,
			}),
		);
	});
});

describe("loadEpochsState", () => {
	it("returns epoch state with group IDs", async () => {
		const provider = {
			readContract: vi.fn().mockImplementation(({ functionName, args }: { functionName: string; args?: unknown[] }) => {
				if (functionName === "getEpochsState") {
					return [1n, 2n, 3n, 500n];
				}
				if (functionName === "getEpochGroupId") {
					const epoch = (args as bigint[])[0];
					if (epoch === 2n) return GROUP_ID_A;
					if (epoch === 3n) return GROUP_ID_B;
				}
				return undefined;
			}),
		} as unknown as PublicClient;

		const state = await loadEpochsState(provider, CONSENSUS);
		expect(state).toEqual({
			previous: 1n,
			active: 2n,
			staged: 3n,
			rolloverBlock: 500n,
			activeGroupId: GROUP_ID_A,
			stagedGroupId: GROUP_ID_B,
		});
	});

	it("returns null stagedGroupId when staged epoch is 0", async () => {
		const provider = {
			readContract: vi.fn().mockImplementation(({ functionName, args }: { functionName: string; args?: unknown[] }) => {
				if (functionName === "getEpochsState") {
					return [0n, 1n, 0n, 0n];
				}
				if (functionName === "getEpochGroupId") {
					const epoch = (args as bigint[])[0];
					if (epoch === 1n) return GROUP_ID_A;
				}
				return undefined;
			}),
		} as unknown as PublicClient;

		const state = await loadEpochsState(provider, CONSENSUS);
		expect(state.stagedGroupId).toBeNull();
		expect(state.activeGroupId).toBe(GROUP_ID_A);
	});
});
