import type { Address, Hex, PublicClient } from "viem";
import { numberToHex } from "viem";
import { describe, expect, it, vi } from "vitest";
import { loadEpochRolloverHistory, loadEpochsState, loadTransactionProposals } from "./consensus";

const CONSENSUS = "0x0000000000000000000000000000000000000001" as Address;
const SAFE_TX_HASH = `0x${"ab".repeat(32)}` as Hex;
const SAFE_ADDRESS = "0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF" as Address;
const CURRENT_BLOCK = 10000n;
const MAX_BLOCK_RANGE = 1000n;

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
			await loadTransactionProposals({ provider, consensus: CONSENSUS, maxBlockRange: MAX_BLOCK_RANGE });
			expect((provider.request as ReturnType<typeof vi.fn>).mock.calls).toHaveLength(1);
		});

		it("passes both event selectors as an OR filter in topic[0]", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({ provider, consensus: CONSENSUS, maxBlockRange: MAX_BLOCK_RANGE });
			expect(Array.isArray(firstCall(provider).topics[0])).toBe(true);
			expect(firstCall(provider).topics[0]).toHaveLength(2);
		});

		it("uses null for topic[1] when safeTxHash is not provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({ provider, consensus: CONSENSUS, maxBlockRange: MAX_BLOCK_RANGE });
			expect(firstCall(provider).topics[1]).toBeNull();
		});

		it("filters by safeTxHash in topic[1] when provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safeTxHash: SAFE_TX_HASH,
				maxBlockRange: MAX_BLOCK_RANGE,
			});
			expect(firstCall(provider).topics[1]).toBe(SAFE_TX_HASH);
		});

		it("uses null for topic[3] when safe is not provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({ provider, consensus: CONSENSUS, maxBlockRange: MAX_BLOCK_RANGE });
			expect(firstCall(provider).topics[3]).toBeNull();
		});

		it("filters by safe address in topic[3] when provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safe: SAFE_ADDRESS,
				maxBlockRange: MAX_BLOCK_RANGE,
			});
			expect(firstCall(provider).topics[2]).toBeNull(); // chainId wildcard
			expect(firstCall(provider).topics[3]).toBe(SAFE_ADDRESS);
		});
	});

	describe("block range", () => {
		it("always includes an explicit toBlock in the request", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({ provider, consensus: CONSENSUS, maxBlockRange: MAX_BLOCK_RANGE });
			expect(firstCall(provider).toBlock).toBe(numberToHex(CURRENT_BLOCK));
		});

		it("does not call getBlockNumber when toBlock is provided", async () => {
			const provider = makeProvider();
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				toBlock: 6000n,
				maxBlockRange: MAX_BLOCK_RANGE,
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
			});
			expect(result.fromBlock).toBe(5000n);
			expect(result.toBlock).toBe(6000n);
			expect(result.proposals).toEqual([]);
		});
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
