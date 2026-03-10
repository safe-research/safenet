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

// A minimal raw log stub that carries a tx hash in topics[1] so the implementation
// can extract it for the attestation query without needing a fully ABI-encoded log.
const makeRawProposedLog = (txHash: Hex) => ({
	address: CONSENSUS,
	topics: [
		"0x0000000000000000000000000000000000000000000000000000000000000000", // selector placeholder
		txHash,
		`0x${"00".repeat(31)}01`, // chainId = 1
		`0x${"000000000000000000000000"}${SAFE_ADDRESS.slice(2)}`, // safe (padded)
	],
	data: "0x",
	blockNumber: "0x1",
	transactionHash: txHash,
	blockHash: `0x${"00".repeat(32)}`,
	logIndex: "0x0",
	transactionIndex: "0x0",
	removed: false,
});

const makeProvider = (...responses: unknown[][]): PublicClient => {
	const mock = vi.fn();
	for (const response of responses) {
		mock.mockResolvedValueOnce(response);
	}
	mock.mockResolvedValue([]); // default for any additional calls
	return {
		getBlockNumber: vi.fn().mockResolvedValue(CURRENT_BLOCK),
		request: mock,
	} as unknown as PublicClient;
};

const requestCalls = (provider: PublicClient) =>
	(provider.request as ReturnType<typeof vi.fn>).mock.calls.map((c) => c[0].params[0]);

describe("loadTransactionProposals", () => {
	describe("without safe filter — single eth_getLogs call", () => {
		it("makes a single request", async () => {
			const provider = makeProvider([]);
			await loadTransactionProposals({ provider, consensus: CONSENSUS, maxBlockRange: MAX_BLOCK_RANGE });
			expect(requestCalls(provider)).toHaveLength(1);
		});

		it("uses both event selectors as an OR filter in topic[0]", async () => {
			const provider = makeProvider([]);
			await loadTransactionProposals({ provider, consensus: CONSENSUS, maxBlockRange: MAX_BLOCK_RANGE });
			expect(Array.isArray(requestCalls(provider)[0].topics[0])).toBe(true);
			expect(requestCalls(provider)[0].topics[0]).toHaveLength(2);
		});

		it("filters by safeTxHash in topic[1] when provided", async () => {
			const provider = makeProvider([]);
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safeTxHash: SAFE_TX_HASH,
				maxBlockRange: MAX_BLOCK_RANGE,
			});
			expect(requestCalls(provider)[0].topics[1]).toBe(SAFE_TX_HASH);
		});
	});

	describe("with safe filter — two eth_getLogs calls", () => {
		it("skips the attestation request when no proposals are found", async () => {
			const provider = makeProvider([]); // first call returns no proposals
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safe: SAFE_ADDRESS,
				maxBlockRange: MAX_BLOCK_RANGE,
			});
			expect(requestCalls(provider)).toHaveLength(1);
		});

		it("makes a second request for attestations when proposals are found", async () => {
			const provider = makeProvider([makeRawProposedLog(SAFE_TX_HASH)]); // first call returns one proposal
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safe: SAFE_ADDRESS,
				maxBlockRange: MAX_BLOCK_RANGE,
			});
			expect(requestCalls(provider)).toHaveLength(2);
		});

		it("first call uses a single selector and filters by safe in topic[3]", async () => {
			const provider = makeProvider([]);
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safe: SAFE_ADDRESS,
				maxBlockRange: MAX_BLOCK_RANGE,
			});
			const first = requestCalls(provider)[0];
			expect(Array.isArray(first.topics[0])).toBe(false); // single selector, not OR array
			expect(first.topics[2]).toBeNull(); // chainId wildcard
			expect(first.topics[3]).toBe(SAFE_ADDRESS);
		});

		it("second call uses a single selector with no safe topic", async () => {
			const provider = makeProvider([makeRawProposedLog(SAFE_TX_HASH)]);
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safe: SAFE_ADDRESS,
				maxBlockRange: MAX_BLOCK_RANGE,
			});
			const second = requestCalls(provider)[1];
			expect(Array.isArray(second.topics[0])).toBe(false); // single selector
			expect(second.topics[3]).toBeUndefined(); // no safe topic on TransactionAttested
		});

		it("second call filters by the tx hashes returned by the proposal query", async () => {
			const provider = makeProvider([makeRawProposedLog(SAFE_TX_HASH)]);
			await loadTransactionProposals({
				provider,
				consensus: CONSENSUS,
				safe: SAFE_ADDRESS,
				maxBlockRange: MAX_BLOCK_RANGE,
			});
			expect(requestCalls(provider)[1].topics[1]).toContain(SAFE_TX_HASH);
		});
	});

	describe("block range", () => {
		it("always includes an explicit toBlock in the request", async () => {
			const provider = makeProvider([]);
			await loadTransactionProposals({ provider, consensus: CONSENSUS, maxBlockRange: MAX_BLOCK_RANGE });
			expect(requestCalls(provider)[0].toBlock).toBe(numberToHex(CURRENT_BLOCK));
		});

		it("does not call getBlockNumber when toBlock is provided", async () => {
			const provider = makeProvider([]);
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
			const provider = makeProvider([]);
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
			provider: makeEpochProvider(),
			consensus: CONSENSUS,
			maxBlockRange: 500n,
		});
		expect(result.entries).toEqual([]);
		expect(result.reachedGenesis).toBe(false);
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

	it("uses cursor as toBlock when provided", async () => {
		const provider = makeEpochProvider();
		await loadEpochRolloverHistory({
			provider,
			consensus: CONSENSUS,
			maxBlockRange: 500n,
			cursor: 800n,
		});
		expect(provider.getLogs).toHaveBeenCalledWith(
			expect.objectContaining({
				toBlock: 800n,
			}),
		);
		expect(provider.getBlockNumber).not.toHaveBeenCalled();
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
